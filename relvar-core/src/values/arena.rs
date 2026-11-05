//! Waymaker-style arena value layer: zero-alloc values for the relational model.
//!
//! This module implements the v0.8 "true zero-alloc values" tier: relations as
//! fixed-capacity slot arrays, tuples as borrowed views, and all
//! variable-length data living in caller-owned bump arenas. It is a **new,
//! additive API** — the existing owned value layer ([`ScalarValue`], [`Tuple`],
//! [`Relation`]) is untouched.
//!
//! # Design
//!
//! - [`Arena`] is a bump allocator over fixed-capacity chunks (4 KiB default),
//!   growing by allocating more chunks. Allocation methods take `&self` and use
//!   interior mutability, so any number of borrowed values can be alive at
//!   once; `reset` takes `&mut self`, so the borrow checker statically forbids
//!   resetting (or dropping) the arena while any borrowed value is live. This
//!   is the same shape as `bumpalo`.
//! - [`ScalarRef`] mirrors [`ScalarValue`] but borrows variable-length data
//!   (`Text(&str)`, `Binary(&[u8])`) instead of owning it. Numerics and bools
//!   are `Copy` inline.
//! - [`TupleView`] is a borrowed tuple: a heading reference plus one
//!   arena-allocated slice of [`ScalarRef`]s, positional in heading attribute
//!   order.
//! - [`SlotRelation`] is a fixed-capacity set of tuple views. The arena
//!   and the heading are caller-owned borrows (`&'a Arena`, `&'a TupleType`),
//!   so no raw pointers or `unsafe` appear outside [`Arena`] itself.
//!
//! # `unsafe`
//!
//! All `unsafe` in this module lives in [`Arena`]'s allocation methods —
//! five blocks, one each in [`Arena::alloc_bytes`], [`Arena::alloc_str`],
//! [`Arena::alloc_slice`], [`Arena::alloc_slice_mut`] and
//! [`Arena::alloc_value`]. They turn a raw pointer from the private
//! `alloc_raw` into a typed borrow tied to `&self`; the invariants they rely
//! on are listed on [`Arena`], and each block carries a `// SAFETY:` comment
//! naming the ones it uses (enforced by `clippy::undocumented_unsafe_blocks`).
//!
//! # `&self` vs `&mut self` on the allocator
//!
//! An earlier sketch of this API used `&mut self` allocation methods. That
//! shape cannot express the required conversions: building one [`TupleView`]
//!   needs many live borrows from the same arena (one per text/binary value
//!   plus the value slice), and `&mut self` methods cannot be called again
//!   while an earlier borrow is live. The `&self` + interior-mutability design
//!   used here keeps every borrow shared, so the borrow checker is satisfied
//!   and `reset(&mut self)` still statically invalidates outstanding borrows.
//!
//! # TTM notes
//!
//! - **Proscription 1** (no NULLs): every attribute always has a value; there
//!   is no null representation anywhere in this layer.
//! - **Proscription 2** (no duplicates): [`SlotRelation`] is a set.
//!   [`SlotRelation::insert`] treats an already-present tuple as a no-op
//!   (like [`Relation::insert`]), [`SlotRelation::from_relation`] copies a
//!   set, and equality / hashing are set-based.
//! - **Every tuple conforms to its heading**: [`SlotRelation::insert`] and
//!   [`TupleView::new`] type-check values against the heading, and
//!   [`TupleView`]'s fields are private, so no view can disagree with its
//!   heading.
//! - **Proscription 4** (no attribute ordering): [`TupleView`] equality and
//!   hashing use attribute-set semantics; the positional slice order is an
//!   internal canonical layout, not a logical ordering.
//!
//! # Example
//!
//! ```
//! use relvar_core::tuple;
//! use relvar_core::types::{RelationType, ScalarType, TupleType};
//! use relvar_core::values::{Arena, Relation, SlotRelation};
//!
//! let heading = TupleType::new()
//!     .with_attribute("id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//!
//! let mut owned = Relation::new(RelationType::new(heading));
//! owned.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
//! owned.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
//!
//! // Move the relation into caller-owned arena storage.
//! let arena = Arena::new();
//! let slots = SlotRelation::from_relation(&owned, &arena, 8).unwrap();
//! assert_eq!(slots.len(), 2);
//!
//! let view = slots.iter().next().unwrap();
//! assert_eq!(view.len(), 2);
//! assert!(view.get("id").is_some());
//!
//! // Round-trip back to owned values.
//! let back = slots.to_relation().unwrap();
//! assert_eq!(back.cardinality(), 2);
//! ```

// Every `unsafe` block must state the invariant it relies on.
#![deny(clippy::undocumented_unsafe_blocks)]

use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue, Tuple};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use thiserror::Error;

use super::relation::RelationError;
use super::tuple::TupleError;

/// Default chunk size for [`Arena`]: 4 KiB, matching the storage page size.
const DEFAULT_CHUNK_SIZE: usize = 4096;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from arena allocation.
///
/// The arena grows by allocating new chunks on demand, so the only failure
/// mode is a request whose size cannot be represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AllocError {
    /// The requested allocation size overflowed `usize` (or exceeded what
    /// [`core::alloc::Layout`] accepts). Carries the offending byte size, or
    /// the element count for slice allocations.
    #[error("allocation size overflow: {0}")]
    SizeOverflow(usize),
}

/// Errors from inserting a tuple into a [`SlotRelation`], or from building a
/// [`TupleView`] with [`TupleView::new`].
///
/// Besides resource limits ([`Full`](Self::Full), [`Alloc`](Self::Alloc)),
/// this reports values that do not conform to the heading
/// ([`ArityMismatch`](Self::ArityMismatch),
/// [`TypeMismatch`](Self::TypeMismatch)): every tuple of a relation conforms
/// to its heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CapacityError {
    /// The slot array is full; no more tuples fit.
    #[error("slot relation is full: capacity {capacity} reached")]
    Full {
        /// The capacity that was exhausted.
        capacity: usize,
    },

    /// The value slice did not match the heading's arity.
    ///
    /// [`SlotRelation::insert`] takes values positionally in heading attribute
    /// order, so the slice length must equal the heading degree exactly.
    #[error("arity mismatch: heading has {expected} attributes but {found} values were given")]
    ArityMismatch {
        /// The heading's degree.
        expected: usize,
        /// The number of values supplied.
        found: usize,
    },

    /// A value did not conform to its attribute's declared type.
    ///
    /// `position` indexes the value slice, which is positional in heading
    /// attribute order. A user-defined value conforms only if its type is the
    /// attribute's type *and* its representation conforms to that type's
    /// representation; a relation value only if its heading is the
    /// attribute's relation heading.
    #[error("type mismatch: value at position {position} does not conform to its attribute type")]
    TypeMismatch {
        /// The offending value's position (heading attribute order).
        position: usize,
    },

    /// An arena allocation backing the insert failed.
    #[error("arena allocation failed: {0}")]
    Alloc(#[from] AllocError),
}

/// Errors from [`SlotRelation::from_relation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SlotRelationError {
    /// The source relation did not fit in the requested capacity.
    #[error("slot relation capacity error: {0}")]
    Capacity(#[from] CapacityError),

    /// An arena allocation backing the conversion failed.
    #[error("arena allocation failed: {0}")]
    Alloc(#[from] AllocError),
}

/// Errors from materializing arena-borrowed values back into owned values
/// ([`ScalarRef::to_owned`], [`TupleView::to_tuple`], [`SlotRelation::to_relation`]).
///
/// Views can only be built through validating constructors
/// ([`TupleView::new`], [`TupleView::from_tuple`], [`SlotRelation::insert`],
/// [`SlotRelation::from_relation`]), so these do not fire in practice; they
/// exist so materialization never panics.
#[derive(Debug, Error)]
pub enum MaterializeError {
    /// A tuple could not be rebuilt from the view.
    #[error("tuple materialization failed: {0}")]
    Tuple(#[from] TupleError),

    /// A tuple could not be inserted into the materialized relation.
    #[error("relation materialization failed: {0}")]
    Relation(#[from] RelationError),
}

// ---------------------------------------------------------------------------
// Arena: a no_std + alloc bump allocator
// ---------------------------------------------------------------------------

/// One fixed-capacity bump chunk.
///
/// The backing buffer is a `Vec<u8>` that is created at its final length and
/// never resized, so its heap bytes never move: moving the `Vec` itself (when
/// the chunk list reallocates) moves only its header.
///
/// Aliasing discipline: once a chunk exists, its bytes are only ever reached
/// through the raw pointer [`Vec::as_mut_ptr`] returns, which is guaranteed
/// not to materialize a reference to the buffer. No `&[u8]`/`&mut [u8]` over
/// the whole buffer is ever created, so carving a new allocation can never
/// invalidate (or alias) a `&mut` previously handed out from the same chunk.
#[derive(Debug)]
struct Chunk {
    storage: Vec<u8>,
}

impl Chunk {
    /// A zero-filled chunk of exactly `len` bytes.
    fn new(len: usize) -> Self {
        Self {
            storage: alloc::vec![0u8; len],
        }
    }

    /// The chunk length in bytes (reads only the `Vec` header).
    fn len(&self) -> usize {
        self.storage.len()
    }

    /// Raw base pointer of the chunk, with provenance over the whole buffer.
    ///
    /// Takes `&mut self` because the pointer is used for writes;
    /// [`Vec::as_mut_ptr`] creates no reference to the bytes, so pointers
    /// obtained earlier (and borrows derived from them) stay valid.
    fn base_ptr(&mut self) -> *mut u8 {
        self.storage.as_mut_ptr()
    }
}

/// Places `size` bytes aligned to `align` in a chunk of `len` bytes whose
/// base address is `base_addr`, starting the search at `cursor`.
///
/// Returns `(offset, new_cursor)`: the allocation's offset from the chunk
/// base and the cursor just past it, or `None` if it does not fit. Alignment
/// is applied to the absolute address, so it holds regardless of the chunk's
/// own alignment. `align` must be a non-zero power of two.
fn place(
    base_addr: usize,
    cursor: usize,
    len: usize,
    size: usize,
    align: usize,
) -> Option<(usize, usize)> {
    let addr = base_addr.checked_add(cursor)?;
    // Bytes from `addr` up to the next multiple of `align`.
    let padding = addr.wrapping_neg() & (align - 1);
    let offset = cursor.checked_add(padding)?;
    let end = offset.checked_add(size)?;
    (end <= len).then_some((offset, end))
}

/// A bump (arena) allocator for zero-alloc values.
///
/// Memory is handed out of fixed-capacity chunks (4 KiB by default) with a
/// monotonically increasing cursor; when a chunk is full a new one is
/// allocated. Memory is never freed individually — call [`Arena::reset`] to
/// rewind the whole arena for reuse.
///
/// Allocation methods take `&self` (interior mutability via [`Cell`]/[`RefCell`])
/// so that any number of borrowed values can be alive simultaneously. This is
/// sound because every allocation returns a disjoint, never-reused region, and
/// `reset` requires `&mut self`, which the borrow checker forbids while any
/// borrow is live.
///
/// The `unsafe` blocks in the allocation methods ([`alloc_bytes`],
/// [`alloc_str`], [`alloc_slice`], [`alloc_slice_mut`], [`alloc_value`]) rely
/// on these invariants, which the private `alloc_raw` establishes:
///
/// - The returned pointer is non-null, aligned as requested, and (for a
///   non-zero size) in bounds of one chunk with that chunk's provenance.
/// - Allocations are carved from a monotonically increasing cursor, so no two
///   live allocations overlap.
/// - Chunk byte buffers are heap-stable: pushing a new chunk may reallocate
///   the chunk list, but that moves the chunks' `Vec` headers, never the
///   bytes they own (chunk buffers are never resized).
/// - Chunk bytes are only reached through raw pointers ([`Vec::as_mut_ptr`]);
///   no reference over a whole chunk is ever created, so handing out a new
///   region never aliases or invalidates an earlier one.
/// - `reset` and `drop` require `&mut self`; the returned references are tied
///   to the `&self` borrow in the method signatures, so the borrow checker
///   statically prevents invalidation while borrows are live.
/// - Arena bytes are not assumed to be initialized: after `reset` a reused
///   chunk may hold the uninitialized padding of earlier allocations. Every
///   method therefore writes each byte or value before it forms a reference
///   over it.
///
/// Values moved in with [`alloc_value`] are never dropped (neither on
/// `reset` nor when the arena is dropped); any heap memory they own is
/// leaked.
///
/// [`alloc_bytes`]: Arena::alloc_bytes
/// [`alloc_str`]: Arena::alloc_str
/// [`alloc_slice`]: Arena::alloc_slice
/// [`alloc_slice_mut`]: Arena::alloc_slice_mut
/// [`alloc_value`]: Arena::alloc_value
///
/// `Arena` is `!Sync` (via [`RefCell`]) — arenas are single-threaded, matching
/// the Waymaker caller-owned-arena discipline.
#[derive(Debug)]
pub struct Arena {
    chunks: RefCell<Vec<Chunk>>,
    /// Bump cursor into the *last* chunk.
    cursor: Cell<usize>,
    chunk_size: usize,
}

impl Arena {
    /// Creates an arena with the default 4 KiB chunk size.
    pub fn new() -> Self {
        Self::with_chunk_size(DEFAULT_CHUNK_SIZE)
    }

    /// Creates an arena with a custom chunk size in bytes.
    ///
    /// Sizes below 1 byte are clamped to 1. Single allocations larger than the
    /// chunk size are served from a dedicated oversized chunk.
    pub fn with_chunk_size(chunk_bytes: usize) -> Self {
        let chunk_size = chunk_bytes.max(1);
        Self {
            chunks: RefCell::new(alloc::vec![Chunk::new(chunk_size)]),
            cursor: Cell::new(0),
            chunk_size,
        }
    }

    /// The chunk size this arena was configured with, in bytes.
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Total bytes handed out across all chunks (the sum of the bump cursors).
    pub fn used_bytes(&self) -> usize {
        let chunks = self.chunks.borrow();
        let last = chunks.len().saturating_sub(1);
        chunks
            .iter()
            .enumerate()
            .map(|(i, chunk)| {
                if i == last {
                    self.cursor.get()
                } else {
                    chunk.len()
                }
            })
            .sum()
    }

    /// Total bytes reserved across all chunks.
    pub fn capacity_bytes(&self) -> usize {
        self.chunks.borrow().iter().map(Chunk::len).sum()
    }

    /// Rewinds the arena for reuse: frees all but the first chunk and resets
    /// the bump cursor to zero.
    ///
    /// Takes `&mut self`, so the borrow checker guarantees no borrowed value
    /// is live across the call — previously handed-out references are
    /// logically invalidated.
    pub fn reset(&mut self) {
        // `&mut self` proves exclusive access; `get_mut` skips the runtime check.
        let chunks = self.chunks.get_mut();
        chunks.truncate(1);
        self.cursor.set(0);
    }

    /// Allocates `n` zero-filled bytes and returns them as a mutable slice.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError::SizeOverflow`] if `n` cannot be represented in a
    /// [`core::alloc::Layout`].
    // `&self -> &mut` is the bumpalo pattern: every allocation hands out a
    // disjoint region (the bump cursor only advances), so two live `&mut`s can
    // never alias the same bytes. `reset` takes `&mut self`, so the arena
    // cannot be reset while any borrow is live.
    #[allow(clippy::mut_from_ref)]
    pub fn alloc_bytes(&self, n: usize) -> Result<&mut [u8], AllocError> {
        if n == 0 {
            return Ok(&mut []);
        }
        let ptr = self.alloc_raw(n, 1)?;
        // SAFETY: `alloc_raw` returned a non-null pointer to `n` in-bounds
        // bytes of one chunk, disjoint from every other live allocation, so
        // writing them and handing out `&mut` aliases nothing. The bytes may
        // be uninitialized (reused after `reset`), so they are zeroed before
        // the slice is formed. The chunk outlives the `&self` borrow the
        // slice is tied to: chunk buffers never move, and `reset`/`drop`
        // require `&mut self`.
        unsafe {
            core::ptr::write_bytes(ptr, 0, n);
            Ok(core::slice::from_raw_parts_mut(ptr, n))
        }
    }

    /// Copies `s` into the arena and returns it as a borrowed `&str`.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError::SizeOverflow`] if the string does not fit in a
    /// [`core::alloc::Layout`].
    pub fn alloc_str(&self, s: &str) -> Result<&str, AllocError> {
        let bytes = self.alloc_bytes(s.len())?;
        bytes.copy_from_slice(s.as_bytes());
        // SAFETY: `from_utf8_unchecked` requires valid UTF-8; `bytes` is an
        // exact copy of `s`, which is a `&str` and hence valid UTF-8.
        Ok(unsafe { core::str::from_utf8_unchecked(bytes) })
    }

    /// Copies `items` into the arena with proper alignment for `T` and returns
    /// the copy as a borrowed slice.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError::SizeOverflow`] if `items.len() *
    /// size_of::<T>()` overflows or cannot be represented in a
    /// [`core::alloc::Layout`].
    pub fn alloc_slice<T: Copy>(&self, items: &[T]) -> Result<&[T], AllocError> {
        if items.is_empty() {
            return Ok(&[]);
        }
        let bytes = items
            .len()
            .checked_mul(core::mem::size_of::<T>())
            .ok_or(AllocError::SizeOverflow(items.len()))?;
        let ptr = self.alloc_raw(bytes, core::mem::align_of::<T>())? as *mut T;
        // SAFETY: `alloc_raw` returned a non-null pointer, aligned for `T`,
        // to `items.len() * size_of::<T>()` in-bounds bytes of one chunk,
        // disjoint from `items` and from every other live allocation (so
        // `copy_nonoverlapping` is valid). `T: Copy` makes a bitwise copy a
        // valid `T`, and every slot is written before the shared slice is
        // formed. The slice is tied to `&self`, which keeps the chunk alive
        // and unmoved (see `alloc_bytes`).
        unsafe {
            core::ptr::copy_nonoverlapping(items.as_ptr(), ptr, items.len());
            Ok(core::slice::from_raw_parts(ptr as *const T, items.len()))
        }
    }

    /// Allocates a mutable slice of `len` copies of `value`, with `T`'s alignment.
    ///
    /// This is the exclusive-access counterpart to [`Arena::alloc_slice`]: the
    /// caller receives `&mut [T]` to fill in place. It backs arena-resident
    /// hash-table buckets and reusable scratch buffers in the algebra layer
    /// ([`crate::algebra::arena_ops`]), keeping the zero-alloc operator
    /// pipeline free of heap allocation beyond the arena bump.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError::SizeOverflow`] if `len * size_of::<T>()`
    /// overflows or cannot be represented in a [`core::alloc::Layout`].
    // See `alloc_bytes`: disjoint regions per allocation make `&self -> &mut`
    // sound here.
    #[allow(clippy::mut_from_ref)]
    pub fn alloc_slice_mut<T: Copy>(&self, len: usize, value: T) -> Result<&mut [T], AllocError> {
        if len == 0 {
            return Ok(&mut []);
        }
        let bytes = len
            .checked_mul(core::mem::size_of::<T>())
            .ok_or(AllocError::SizeOverflow(len))?;
        let ptr = self.alloc_raw(bytes, core::mem::align_of::<T>())? as *mut T;
        // SAFETY: `alloc_raw` returned a non-null pointer, aligned for `T`,
        // to `len * size_of::<T>()` in-bounds bytes of one chunk, disjoint
        // from every other live allocation, so the `&mut` aliases nothing.
        // Each of the `len` slots is initialized with `ptr::write` (the old
        // bytes may be uninitialized; `T: Copy` has no drop to skip) before
        // the slice is formed, and `i < len` keeps every `add` in bounds.
        // The slice is tied to `&self`, which keeps the chunk alive and
        // unmoved (see `alloc_bytes`).
        unsafe {
            for i in 0..len {
                ptr.add(i).write(value);
            }
            Ok(core::slice::from_raw_parts_mut(ptr, len))
        }
    }

    /// Moves `value` into arena memory and returns an exclusive borrow of it.
    ///
    /// Used to place non-`Copy` structures (nested [`SlotRelation`]s, nested
    /// [`ScalarRef`]s, cloned [`ScalarType`]s) inside the arena during
    /// [`ScalarRef::from_owned`] conversion.
    ///
    /// `value` is never dropped — not on [`Arena::reset`] and not when the
    /// arena is dropped — so any heap memory it owns is leaked.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError::SizeOverflow`] if `T` cannot be represented in a
    /// [`core::alloc::Layout`].
    // See `alloc_bytes`: disjoint regions per allocation make `&self -> &mut`
    // sound here.
    #[allow(clippy::mut_from_ref)]
    pub fn alloc_value<T>(&self, value: T) -> Result<&mut T, AllocError> {
        let ptr = self.alloc_raw(core::mem::size_of::<T>(), core::mem::align_of::<T>())? as *mut T;
        // SAFETY: `alloc_raw` returned a non-null pointer, aligned for `T`,
        // to `size_of::<T>()` in-bounds bytes of one chunk, disjoint from
        // every other live allocation, so the `&mut` aliases nothing. The
        // slot is initialized by `ptr::write` (which does not read or drop
        // the possibly-uninitialized old bytes) before the borrow is formed.
        // For zero-sized `T` the pointer is dangling but non-null and
        // aligned (see `alloc_raw`) and no memory is touched. The borrow is
        // tied to `&self`, which keeps the chunk alive and unmoved (see
        // `alloc_bytes`).
        unsafe {
            core::ptr::write(ptr, value);
            Ok(&mut *ptr)
        }
    }

    /// Carves `size` bytes with `align` alignment out of the bump region.
    ///
    /// Returns a raw pointer; callers wrap it in a borrow tied to `&self`.
    /// Alignment is applied to the absolute address (chunk base + cursor), so
    /// it holds regardless of the chunk's own alignment.
    fn alloc_raw(&self, size: usize, align: usize) -> Result<*mut u8, AllocError> {
        // Validates that `align` is a non-zero power of two and that `size`
        // fits the platform's layout rules; the manual arithmetic below relies
        // on both. Done first so zero-size requests are validated too.
        let _ = core::alloc::Layout::from_size_align(size, align)
            .map_err(|_| AllocError::SizeOverflow(size))?;
        if size == 0 {
            // A zero-size request touches no memory, but callers cast the
            // pointer to `*mut T` (zero-sized `T`), so it must be non-null
            // and aligned for `align`: the address `align` itself is both
            // (`align` is a non-zero power of two). This is exactly what
            // `NonNull::<T>::dangling()` produces for `align_of::<T>()`.
            return Ok(core::ptr::without_provenance_mut(align));
        }

        let mut chunks = self.chunks.borrow_mut();

        // Fast path: room in the current (last) chunk.
        if let Some(chunk) = chunks.last_mut() {
            let base = chunk.base_ptr();
            if let Some((offset, new_cursor)) =
                place(base.addr(), self.cursor.get(), chunk.len(), size, align)
            {
                self.cursor.set(new_cursor);
                // In bounds (`new_cursor <= len`); `wrapping_add` keeps the
                // chunk's provenance without an `unsafe` offset.
                return Ok(base.wrapping_add(offset));
            }
        }

        // Slow path: the current chunk is full (or missing); grow.
        // Worst-case padding is `align - 1`, so `size + align` always fits.
        let need = size
            .checked_add(align)
            .ok_or(AllocError::SizeOverflow(size))?;
        let chunk_len = self.chunk_size.max(need);
        let mut chunk = Chunk::new(chunk_len);
        // Taken before the chunk moves into the list: moving the `Vec`
        // header does not move (or retag) its heap bytes.
        let base = chunk.base_ptr();
        let (offset, new_cursor) =
            place(base.addr(), 0, chunk_len, size, align).ok_or(AllocError::SizeOverflow(size))?;
        chunks.push(chunk);
        self.cursor.set(new_cursor);
        Ok(base.wrapping_add(offset))
    }
}

impl Default for Arena {
    /// Creates an arena with the default 4 KiB chunk size.
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ScalarRef: an arena-borrowed scalar value
// ---------------------------------------------------------------------------

/// An arena-borrowed scalar value.
///
/// Mirrors every [`ScalarValue`] variant, but variable-length data is borrowed
/// from an [`Arena`] instead of owned:
///
/// - Numerics and bools are `Copy` inline: [`Int`](ScalarRef::Int),
///   [`Float`](ScalarRef::Float), [`Bool`](ScalarRef::Bool).
/// - [`Text`](ScalarRef::Text) borrows a `&str`, [`Binary`](ScalarRef::Binary)
///   borrows a `&[u8]`.
/// - [`Relation`](ScalarRef::Relation) borrows a nested [`SlotRelation`]
///   (relation-valued attributes).
/// - [`UserDefined`](ScalarRef::UserDefined) borrows the cloned
///   [`ScalarType`] and the nested borrowed representation value.
///
/// `f64` uses bit equality (`NaN == NaN`), matching [`ScalarValue`]'s set
/// semantics, so the [`PartialEq`]/[`Eq`]/[`Hash`] impls are manual.
#[derive(Clone, Copy, Debug)]
pub enum ScalarRef<'a> {
    /// 64-bit signed integer value.
    Int(i64),

    /// 64-bit floating point value (bit equality: `NaN == NaN`).
    Float(f64),

    /// Boolean value.
    Bool(bool),

    /// UTF-8 string value borrowed from the arena.
    Text(&'a str),

    /// Arbitrary byte sequence borrowed from the arena.
    Binary(&'a [u8]),

    /// Relation-valued attribute: a nested slot relation borrowed from the arena.
    Relation(&'a SlotRelation<'a>),

    /// User-defined (POSSREP) value: the borrowed type definition plus the
    /// borrowed representation value.
    UserDefined {
        /// The borrowed type definition (cloned into the arena on conversion).
        type_def: &'a ScalarType,
        /// The borrowed representation value (placed in the arena on conversion).
        value: &'a ScalarRef<'a>,
    },
}

// Manual PartialEq: f64 needs bit equality for set semantics (NaN == NaN),
// mirroring ScalarValue.
impl<'a> PartialEq for ScalarRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        if core::mem::discriminant(self) != core::mem::discriminant(other) {
            return false;
        }
        match (self, other) {
            (ScalarRef::Int(a), ScalarRef::Int(b)) => a == b,
            (ScalarRef::Float(a), ScalarRef::Float(b)) => {
                if a.is_nan() && b.is_nan() {
                    true
                } else {
                    a.to_bits() == b.to_bits()
                }
            }
            (ScalarRef::Bool(a), ScalarRef::Bool(b)) => a == b,
            (ScalarRef::Text(a), ScalarRef::Text(b)) => a == b,
            (ScalarRef::Binary(a), ScalarRef::Binary(b)) => a == b,
            (ScalarRef::Relation(a), ScalarRef::Relation(b)) => a == b,
            (
                ScalarRef::UserDefined {
                    type_def: type_a,
                    value: val_a,
                },
                ScalarRef::UserDefined {
                    type_def: type_b,
                    value: val_b,
                },
            ) => type_a == type_b && val_a == val_b,
            // The discriminant check above makes this unreachable; `false` is
            // defensive (no panics in this layer).
            _ => false,
        }
    }
}

impl<'a> Eq for ScalarRef<'a> {}

// Manual Hash: mirrors ScalarValue's discriminant bytes (Int=0, Float=1,
// Text=2, Bool=3, Binary=4, Relation=5, UserDefined=6) with canonical NaN bits.
impl<'a> core::hash::Hash for ScalarRef<'a> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        match self {
            ScalarRef::Int(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            ScalarRef::Float(v) => {
                1u8.hash(state);
                if v.is_nan() {
                    f64::NAN.to_bits().hash(state);
                } else {
                    v.to_bits().hash(state);
                }
            }
            ScalarRef::Text(v) => {
                2u8.hash(state);
                v.hash(state);
            }
            ScalarRef::Bool(v) => {
                3u8.hash(state);
                v.hash(state);
            }
            ScalarRef::Binary(v) => {
                4u8.hash(state);
                v.hash(state);
            }
            ScalarRef::Relation(v) => {
                5u8.hash(state);
                v.hash(state);
            }
            ScalarRef::UserDefined { type_def, value } => {
                6u8.hash(state);
                type_def.hash(state);
                value.hash(state);
            }
        }
    }
}

impl<'a> ScalarRef<'a> {
    /// Checks if this value is of the given type, without allocating.
    ///
    /// Mirrors [`ScalarValue::is_type`]: a relation value conforms when its
    /// heading equals the type's heading; a user-defined value conforms when
    /// its type definition equals `ty` and its representation value conforms
    /// to `ty`'s representation type (so a malformed POSSREP value is never
    /// of any type).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::ScalarType;
    /// use relvar_core::values::ScalarRef;
    ///
    /// assert!(ScalarRef::Int(1).is_type(&ScalarType::Int));
    /// assert!(ScalarRef::Text("a").is_type(&ScalarType::String));
    /// assert!(!ScalarRef::Int(1).is_type(&ScalarType::Float));
    ///
    /// let widget = ScalarType::user_defined("WidgetId", ScalarType::Int);
    /// let one = ScalarRef::Int(1);
    /// let id = ScalarRef::UserDefined { type_def: &widget, value: &one };
    /// assert!(id.is_type(&widget));
    /// assert!(!one.is_type(&widget));
    /// ```
    pub fn is_type(&self, ty: &ScalarType) -> bool {
        match (self, ty) {
            (ScalarRef::Int(_), ScalarType::Int)
            | (ScalarRef::Float(_), ScalarType::Float)
            | (ScalarRef::Bool(_), ScalarType::Bool)
            | (ScalarRef::Text(_), ScalarType::String)
            | (ScalarRef::Binary(_), ScalarType::Bytes) => true,
            (ScalarRef::Relation(relation), ScalarType::Relation(relation_type)) => {
                relation.heading() == relation_type.heading()
            }
            (
                ScalarRef::UserDefined { type_def, value },
                ScalarType::UserDefined { representation, .. },
            ) => *type_def == ty && value.is_type(representation),
            _ => false,
        }
    }

    /// Borrows an owned [`ScalarValue`] into the arena.
    ///
    /// Fixed-size values are copied inline; text/binary payloads are copied
    /// into the arena; nested relations are converted recursively into a
    /// [`SlotRelation`] placed in the arena; user-defined values clone their
    /// [`ScalarType`] into the arena (a one-time conversion cost) and recurse
    /// into the representation value.
    ///
    /// The source value must outlive `'a`: relation-valued attributes retain a
    /// borrow of the source relation's heading.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] if an arena allocation fails.
    pub fn from_owned(value: &'a ScalarValue, arena: &'a Arena) -> Result<Self, AllocError> {
        match value {
            ScalarValue::Int(i) => Ok(ScalarRef::Int(*i)),
            ScalarValue::Float(f) => Ok(ScalarRef::Float(*f)),
            ScalarValue::Bool(b) => Ok(ScalarRef::Bool(*b)),
            ScalarValue::String(s) => Ok(ScalarRef::Text(arena.alloc_str(s)?)),
            ScalarValue::Bytes(b) => Ok(ScalarRef::Binary(arena.alloc_slice(b)?)),
            ScalarValue::Relation(relation) => {
                let slot = SlotRelation::from_relation_exact(relation, arena)?;
                let placed: &'a mut SlotRelation<'a> = arena.alloc_value(slot)?;
                Ok(ScalarRef::Relation(placed))
            }
            ScalarValue::UserDefined { type_def, value } => {
                let inner = Self::from_owned(value, arena)?;
                let inner_ref: &'a mut ScalarRef<'a> = arena.alloc_value(inner)?;
                // The type definition is owned by the source value; clone it
                // into the arena so the borrow lives as long as the arena.
                let type_ref: &'a mut ScalarType = arena.alloc_value((*type_def).clone())?;
                Ok(ScalarRef::UserDefined {
                    type_def: type_ref,
                    value: inner_ref,
                })
            }
        }
    }

    /// Materializes a borrowed value back into an owned [`ScalarValue`].
    ///
    /// This allocates (it leaves the zero-alloc layer); the reverse direction
    /// of [`ScalarRef::from_owned`].
    ///
    /// # Errors
    ///
    /// Returns [`MaterializeError`] if a nested relation or user-defined value
    /// cannot be rebuilt. For views produced by `from_owned` this is
    /// practically infallible.
    pub fn to_owned(&self) -> Result<ScalarValue, MaterializeError> {
        // `*self`: ScalarRef is Copy, so this copies the (reference-sized)
        // enum and gives single-reference bindings, keeping method resolution
        // on the inherent `to_owned` (not the blanket `ToOwned` impl).
        match *self {
            ScalarRef::Int(i) => Ok(ScalarValue::Int(i)),
            ScalarRef::Float(f) => Ok(ScalarValue::Float(f)),
            ScalarRef::Bool(b) => Ok(ScalarValue::Bool(b)),
            ScalarRef::Text(s) => Ok(ScalarValue::String(s.to_string())),
            ScalarRef::Binary(b) => Ok(ScalarValue::Bytes(b.to_vec())),
            ScalarRef::Relation(relation) => Ok(ScalarValue::Relation(relation.to_relation()?)),
            ScalarRef::UserDefined { type_def, value } => Ok(ScalarValue::UserDefined {
                type_def: (*type_def).clone(),
                value: Box::new(value.to_owned()?),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// TupleView: a borrowed tuple
// ---------------------------------------------------------------------------

/// A borrowed tuple: a heading reference plus one arena-allocated slice of
/// [`ScalarRef`]s.
///
/// The values are positional in **heading attribute order** (the order
/// yielded by [`TupleType::attribute_names`]): `values()[i]` is the value of
/// the `i`-th attribute.
///
/// A view always conforms to its heading (one value per attribute, each of
/// its attribute's type; TTM Proscription 1 and RM Prescription 9): the
/// fields are private and every constructor — [`TupleView::new`],
/// [`TupleView::from_tuple`], [`SlotRelation::insert`],
/// [`SlotRelation::from_relation`] — establishes this.
///
/// `Copy` (like `&` references): views are cheap to pass around; the data
/// lives in the arena.
#[derive(Clone, Copy, Debug)]
pub struct TupleView<'a> {
    /// The borrowed heading this view conforms to.
    pub(crate) heading: &'a TupleType,
    /// The attribute values, positional in heading attribute order.
    pub(crate) values: &'a [ScalarRef<'a>],
}

/// Checks that `values` (positional in heading attribute order) conform to
/// `heading`: one value per attribute, each of its attribute's type.
///
/// Allocation-free: one pass over the heading.
fn check_conformance(heading: &TupleType, values: &[ScalarRef<'_>]) -> Result<(), CapacityError> {
    if values.len() != heading.degree() {
        return Err(CapacityError::ArityMismatch {
            expected: heading.degree(),
            found: values.len(),
        });
    }
    for (position, (value, scalar_type)) in
        values.iter().zip(heading.attributes().values()).enumerate()
    {
        if !value.is_type(scalar_type) {
            return Err(CapacityError::TypeMismatch { position });
        }
    }
    Ok(())
}

// Order-independent equality (TTM Proscription 4: no attribute ordering):
// two views are equal when their headings match and every attribute has an
// equal value, regardless of the internal slice order.
impl<'a> PartialEq for TupleView<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.heading == other.heading
            && self.values.len() == other.values.len()
            && self
                .heading
                .attribute_names()
                .all(|name| self.get(name) == other.get(name))
    }
}

impl<'a> Eq for TupleView<'a> {}

// Order-independent hash to match: hash (name, type, value) per attribute in
// the deterministic heading order.
impl<'a> core::hash::Hash for TupleView<'a> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.heading.degree().hash(state);
        for (name, scalar_type) in self.heading.attributes() {
            name.hash(state);
            scalar_type.hash(state);
            if let Some(value) = self.get(name) {
                value.hash(state);
            }
        }
    }
}

impl<'a> TupleView<'a> {
    /// Builds a view over `values`, positional in `heading` attribute order,
    /// after checking that they conform to the heading.
    ///
    /// # Errors
    ///
    /// - [`CapacityError::ArityMismatch`] if `values.len()` is not the
    ///   heading degree.
    /// - [`CapacityError::TypeMismatch`] if a value is not of its attribute's
    ///   type (see [`ScalarRef::is_type`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{ScalarType, TupleType};
    /// use relvar_core::values::{ScalarRef, TupleView};
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let values = [ScalarRef::Int(7)];
    /// let view = TupleView::new(&heading, &values).unwrap();
    /// assert_eq!(view.get("id"), Some(ScalarRef::Int(7)));
    ///
    /// assert!(TupleView::new(&heading, &[ScalarRef::Bool(true)]).is_err());
    /// ```
    pub fn new(heading: &'a TupleType, values: &'a [ScalarRef<'a>]) -> Result<Self, CapacityError> {
        check_conformance(heading, values)?;
        Ok(TupleView { heading, values })
    }

    /// The borrowed heading this view conforms to.
    pub fn heading(&self) -> &'a TupleType {
        self.heading
    }

    /// The attribute values, positional in heading attribute order.
    pub fn values(&self) -> &'a [ScalarRef<'a>] {
        self.values
    }

    /// Borrows an owned [`Tuple`] into the arena.
    ///
    /// Values are stored in heading attribute order (matching
    /// [`Tuple::values`] iteration order, which is attribute-name sorted).
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] if an arena allocation fails.
    pub fn from_tuple(tuple: &'a Tuple, arena: &'a Arena) -> Result<Self, AllocError> {
        let heading: &'a TupleType = tuple.tuple_type();
        // `Tuple::values` iterates in attribute-name order, which is exactly
        // `TupleType::attribute_names` order for a valid tuple.
        let mut buf = Vec::with_capacity(tuple.degree());
        for value in tuple.values().values() {
            buf.push(ScalarRef::from_owned(value, arena)?);
        }
        debug_assert_eq!(buf.len(), heading.degree());
        let values = arena.alloc_slice(&buf)?;
        Ok(TupleView { heading, values })
    }

    /// Materializes the view back into an owned [`Tuple`].
    ///
    /// This allocates (it leaves the zero-alloc layer).
    ///
    /// # Errors
    ///
    /// Returns [`MaterializeError`] if the view violates its construction
    /// contract (e.g. a hand-built view with missing values). Views produced
    /// by [`TupleView::from_tuple`] or [`SlotRelation::insert`] convert
    /// infallibly in practice.
    pub fn to_tuple(&self) -> Result<Tuple, MaterializeError> {
        let mut values = BTreeMap::new();
        for (index, name) in self.heading.attribute_names().enumerate() {
            let scalar_ref = self
                .values
                .get(index)
                .ok_or_else(|| MaterializeError::Tuple(TupleError::MissingValue(name.clone())))?;
            values.insert(name.clone(), scalar_ref.to_owned()?);
        }
        Tuple::new((*self.heading).clone(), values).map_err(MaterializeError::Tuple)
    }

    /// Returns the value of the named attribute, or `None` if the heading has
    /// no such attribute (or the view is short — see [`TupleView::conforms`]).
    ///
    /// Performs no allocation: a linear scan over the heading's attribute
    /// names.
    pub fn get(&self, name: &str) -> Option<ScalarRef<'a>> {
        for (index, attr_name) in self.heading.attribute_names().enumerate() {
            if attr_name == name {
                return self.values.get(index).copied();
            }
        }
        None
    }

    /// The number of values in this view (its degree when conforming).
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether this view holds no values.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Whether the view conforms to its heading: one value per attribute,
    /// each of its attribute's type.
    ///
    /// Allocation-free. Always `true` for views built through the public
    /// constructors, which establish conformance.
    pub fn conforms(&self) -> bool {
        check_conformance(self.heading, self.values).is_ok()
    }
}

// ---------------------------------------------------------------------------
// SlotRelation: a fixed-capacity slot array of tuple views
// ---------------------------------------------------------------------------

/// A fixed-capacity slot array of [`TupleView`]s: the Waymaker-style relation.
///
/// All tuple data lives in the caller-owned [`Arena`]; the slot array itself
/// is one bounded `Vec` allocation (made once, at construction). The arena and
/// the heading are borrowed (`&'a Arena`, `&'a TupleType`), so both must
/// outlive the `SlotRelation` — the borrow checker enforces this at the
/// construction site.
///
/// # Set semantics
///
/// A slot relation is a relation value: its tuples are distinct (TTM
/// Proscription 2) and conform to the heading. [`insert`][Self::insert]
/// type-checks every value and treats an already-present tuple as a no-op,
/// exactly like [`Relation::insert`]; [`from_relation`][Self::from_relation]
/// copies a set. Slot order is physical and carries no meaning
/// (Proscription 3): equality and hashing are set-based.
///
/// Duplicate detection uses an open-addressing index that lives in the arena
/// and is built lazily on the first `insert` (relations only ever read, such
/// as nested relation-valued attributes, never pay for it); it grows by
/// doubling, so inserts are amortized `O(1)`.
pub struct SlotRelation<'a> {
    arena: &'a Arena,
    heading: &'a TupleType,
    slots: Vec<TupleView<'a>>,
    capacity: usize,
    /// Dedup index over `slots` (`None` until the first `insert`).
    index: Option<&'a mut [IndexBucket]>,
}

/// One bucket of a [`SlotRelation`]'s dedup index: the hash of the view in
/// `slots[slot]`, or `slot == EMPTY_SLOT` for an empty bucket.
#[derive(Clone, Copy)]
struct IndexBucket {
    hash: u64,
    slot: usize,
}

/// Marks an empty [`IndexBucket`].
const EMPTY_SLOT: usize = usize::MAX;

impl IndexBucket {
    const EMPTY: Self = Self {
        hash: 0,
        slot: EMPTY_SLOT,
    };
}

/// Smallest dedup-index size: a power of two.
const MIN_INDEX_BUCKETS: usize = 8;

/// Hashes one view with [`TupleView`]'s order-independent `Hash`.
fn view_hash(view: &TupleView<'_>) -> u64 {
    use core::hash::{Hash, Hasher};
    let mut hasher = crate::collections::new_hasher();
    view.hash(&mut hasher);
    hasher.finish()
}

/// Records `(hash, slot)` in an open-addressing table with a free bucket.
fn index_put(buckets: &mut [IndexBucket], hash: u64, slot: usize) {
    let mask = buckets.len() - 1;
    // Truncating the hash is intended: only the low bits pick a bucket.
    let mut at = (hash as usize) & mask;
    while buckets[at].slot != EMPTY_SLOT {
        at = (at + 1) & mask;
    }
    buckets[at] = IndexBucket { hash, slot };
}

impl core::fmt::Debug for SlotRelation<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SlotRelation")
            .field("heading", self.heading)
            .field("slots", &self.slots)
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

// Set semantics for equality (TTM Proscription 2): same heading, same
// cardinality, and every view of one side is present in the other. Slot order
// is physical and ignored. Because a slot relation never holds duplicates,
// equal cardinality plus one-way containment is set equality (and so
// symmetric).
impl<'a> PartialEq for SlotRelation<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.heading == other.heading
            && self.len() == other.len()
            && self.iter().all(|view| other.contains(&view))
    }
}

impl<'a> Eq for SlotRelation<'a> {}

// Order-independent hash mirroring Relation's: heading, cardinality, and the
// XOR of per-view hashes (XOR is commutative, so slot order does not matter;
// views are distinct, so no two hashes cancel as duplicates would).
impl<'a> core::hash::Hash for SlotRelation<'a> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.heading.degree().hash(state);
        for (name, scalar_type) in self.heading.attributes() {
            name.hash(state);
            scalar_type.hash(state);
        }
        self.len().hash(state);
        let mut combined = 0u64;
        for view in self.iter() {
            combined ^= view_hash(&view);
        }
        combined.hash(state);
    }
}

impl<'a> SlotRelation<'a> {
    /// Creates an empty slot relation borrowing `arena` and `heading`, able to
    /// hold up to `capacity` tuple views.
    ///
    /// Both `arena` and `heading` are caller-owned and must outlive the
    /// returned relation.
    pub fn with_capacity(arena: &'a Arena, heading: &'a TupleType, capacity: usize) -> Self {
        Self {
            arena,
            heading,
            slots: Vec::with_capacity(capacity),
            capacity,
            index: None,
        }
    }

    /// The maximum number of tuple views this relation can hold.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The number of tuple views currently stored.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether no tuple views are stored.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The borrowed heading all views conform to.
    pub fn heading(&self) -> &'a TupleType {
        self.heading
    }

    /// Iterates over the stored tuple views (in slot order).
    ///
    /// Views are `Copy`; the iterator itself borrows `self`.
    pub fn iter(&self) -> impl Iterator<Item = TupleView<'a>> + '_ {
        self.slots.iter().copied()
    }

    /// Whether a view equal to `view` is stored.
    ///
    /// Uses the dedup index when one has been built (after the first
    /// [`insert`][Self::insert]); otherwise a linear scan.
    pub fn contains(&self, view: &TupleView<'_>) -> bool {
        match &self.index {
            Some(buckets) => self.find_indexed(buckets, view, view_hash(view)),
            None => self.slots.iter().any(|slot| slot == view),
        }
    }

    /// Probes the dedup index for a stored view equal to `view`.
    fn find_indexed(&self, buckets: &[IndexBucket], view: &TupleView<'_>, hash: u64) -> bool {
        let mask = buckets.len() - 1;
        // Truncating the hash is intended: only the low bits pick a bucket.
        let mut at = (hash as usize) & mask;
        loop {
            let bucket = buckets[at];
            if bucket.slot == EMPTY_SLOT {
                return false;
            }
            if bucket.hash == hash && self.slots.get(bucket.slot).is_some_and(|slot| slot == view) {
                return true;
            }
            at = (at + 1) & mask;
        }
    }

    /// Ensures the dedup index exists and has room for `entries` views at a
    /// load factor of at most 1/2, (re)building it in the arena if needed.
    fn reserve_index(&mut self, entries: usize) -> Result<(), AllocError> {
        let wanted = entries
            .checked_mul(2)
            .ok_or(AllocError::SizeOverflow(entries))?
            .max(MIN_INDEX_BUCKETS);
        if self
            .index
            .as_ref()
            .is_some_and(|buckets| buckets.len() >= wanted)
        {
            return Ok(());
        }
        let len = wanted
            .checked_next_power_of_two()
            .ok_or(AllocError::SizeOverflow(entries))?;
        let buckets = self.arena.alloc_slice_mut(len, IndexBucket::EMPTY)?;
        match self.index.take() {
            // Re-insert from the old table: hashes are already known.
            Some(old) => {
                for bucket in old.iter().filter(|bucket| bucket.slot != EMPTY_SLOT) {
                    index_put(buckets, bucket.hash, bucket.slot);
                }
            }
            None => {
                for (slot, view) in self.slots.iter().enumerate() {
                    index_put(buckets, view_hash(view), slot);
                }
            }
        }
        self.index = Some(buckets);
        Ok(())
    }

    /// Inserts a tuple's values as a new view.
    ///
    /// `values` is positional in **heading attribute order**
    /// (`values[i]` is the value of the `i`-th attribute yielded by
    /// [`TupleType::attribute_names`]). The values are type-checked against
    /// the heading; a tuple that is already present is not inserted again
    /// (set semantics, TTM Proscription 2), mirroring [`Relation::insert`].
    /// A new tuple's slice is copied into the arena and the resulting view is
    /// pushed into the slot array — no per-insert heap allocation beyond the
    /// arena bump.
    ///
    /// Returns `Ok(true)` if the tuple was inserted and `Ok(false)` if an
    /// equal tuple was already present (also when the relation is full).
    ///
    /// # Errors
    ///
    /// - [`CapacityError::ArityMismatch`] if `values.len()` does not equal the
    ///   heading degree.
    /// - [`CapacityError::TypeMismatch`] if a value is not of its attribute's
    ///   type (see [`ScalarRef::is_type`]).
    /// - [`CapacityError::Full`] if the tuple is new and the slot array is at
    ///   capacity (not a panic).
    /// - [`CapacityError::Alloc`] if the arena allocation fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{ScalarType, TupleType};
    /// use relvar_core::values::{Arena, ScalarRef, SlotRelation};
    ///
    /// let heading = TupleType::new().with_attribute("n", ScalarType::Int);
    /// let arena = Arena::new();
    /// let mut slots = SlotRelation::with_capacity(&arena, &heading, 4);
    ///
    /// assert!(slots.insert(&[ScalarRef::Int(1)]).unwrap()); // inserted
    /// assert!(!slots.insert(&[ScalarRef::Int(1)]).unwrap()); // duplicate
    /// assert!(slots.insert(&[ScalarRef::Text("x")]).is_err()); // wrong type
    /// assert_eq!(slots.len(), 1);
    /// ```
    pub fn insert(&mut self, values: &[ScalarRef<'a>]) -> Result<bool, CapacityError> {
        check_conformance(self.heading, values)?;
        let probe = TupleView {
            heading: self.heading,
            values,
        };
        let hash = view_hash(&probe);
        // Room for one more view, but never beyond capacity.
        self.reserve_index((self.slots.len() + 1).min(self.capacity))?;
        if let Some(buckets) = &self.index
            && self.find_indexed(buckets, &probe, hash)
        {
            return Ok(false);
        }
        if self.slots.len() >= self.capacity {
            return Err(CapacityError::Full {
                capacity: self.capacity,
            });
        }
        let stored = self.arena.alloc_slice(values)?;
        let slot = self.slots.len();
        self.slots.push(TupleView {
            heading: self.heading,
            values: stored,
        });
        if let Some(buckets) = self.index.as_deref_mut() {
            index_put(buckets, hash, slot);
        }
        Ok(true)
    }

    /// Removes all tuple views, keeping the capacity.
    ///
    /// The arena is *not* reset — arena memory stays occupied until the caller
    /// explicitly calls [`Arena::reset`].
    pub fn clear(&mut self) {
        self.slots.clear();
        if let Some(buckets) = self.index.as_deref_mut() {
            buckets.fill(IndexBucket::EMPTY);
        }
    }

    /// Borrows an owned [`Relation`] into the arena, with room for up to
    /// `capacity` tuples.
    ///
    /// The source relation must outlive `'a`: views borrow its heading.
    ///
    /// # Errors
    ///
    /// - [`SlotRelationError::Capacity`] if the relation's cardinality exceeds
    ///   `capacity`.
    /// - [`SlotRelationError::Alloc`] if an arena allocation fails.
    pub fn from_relation(
        relation: &'a Relation,
        arena: &'a Arena,
        capacity: usize,
    ) -> Result<Self, SlotRelationError> {
        if relation.cardinality() > capacity {
            return Err(CapacityError::Full { capacity }.into());
        }
        Self::build_from_relation(relation, arena, capacity).map_err(SlotRelationError::Alloc)
    }

    /// Builds a slot relation with capacity exactly equal to the source
    /// relation's cardinality. Used by [`ScalarRef::from_owned`] for nested
    /// relation-valued attributes, where the capacity check cannot fail.
    fn from_relation_exact(relation: &'a Relation, arena: &'a Arena) -> Result<Self, AllocError> {
        let capacity = relation.cardinality();
        Self::build_from_relation(relation, arena, capacity)
    }

    /// Shared conversion core: assumes the caller already checked capacity.
    fn build_from_relation(
        relation: &'a Relation,
        arena: &'a Arena,
        capacity: usize,
    ) -> Result<Self, AllocError> {
        let heading: &'a TupleType = relation.relation_type().heading();
        let mut slots = Self::with_capacity(arena, heading, capacity);
        // One reusable buffer: `Tuple::values` iterates in attribute-name
        // order, matching the positional layout `insert` expects.
        let mut buf = Vec::with_capacity(relation.degree());
        for tuple in relation.tuples() {
            buf.clear();
            for value in tuple.values().values() {
                buf.push(ScalarRef::from_owned(value, arena)?);
            }
            debug_assert_eq!(buf.len(), heading.degree());
            let stored = arena.alloc_slice(&buf)?;
            slots.slots.push(TupleView {
                heading,
                values: stored,
            });
        }
        Ok(slots)
    }

    /// Materializes the slot relation back into an owned [`Relation`].
    ///
    /// This allocates (it leaves the zero-alloc layer). The slot relation is
    /// already a set, so the result has the same cardinality.
    ///
    /// # Errors
    ///
    /// Returns [`MaterializeError`] if a view cannot be rebuilt as an owned
    /// tuple. Every view is built through a validating constructor, so this
    /// is practically infallible.
    pub fn to_relation(&self) -> Result<Relation, MaterializeError> {
        let mut relation =
            Relation::with_capacity(RelationType::new((*self.heading).clone()), self.len());
        for view in self.iter() {
            let tuple = view.to_tuple()?;
            // `insert` only fails on heading mismatch, which cannot happen:
            // every view was built against this heading.
            relation.insert(tuple)?;
        }
        Ok(relation)
    }
}

// ---------------------------------------------------------------------------
// Tests (TDD: written alongside the implementation, red-green-refactor)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collections::HashSet;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use alloc::boxed::Box;
    use core::hash::{Hash, Hasher};

    fn test_heading() -> TupleType {
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("active", ScalarType::Bool)
    }

    fn test_relation() -> Relation {
        let mut relation = Relation::new(RelationType::new(test_heading()));
        relation
            .insert(tuple! { id: 1i64, name: "Alice", active: true })
            .unwrap();
        relation
            .insert(tuple! { id: 2i64, name: "Bob", active: false })
            .unwrap();
        relation
    }

    fn hash_of<T: Hash>(value: &T) -> u64 {
        let mut hasher = crate::collections::new_hasher();
        value.hash(&mut hasher);
        Hasher::finish(&hasher)
    }

    // -- Arena -----------------------------------------------------------

    #[test]
    fn arena_alloc_bytes_write_read() {
        let arena = Arena::new();
        let buf = arena.alloc_bytes(4).unwrap();
        buf.copy_from_slice(&[1, 2, 3, 4]);
        assert_eq!(buf, &[1u8, 2, 3, 4]);
    }

    #[test]
    fn arena_multiple_borrows_coexist() {
        // The `&self` allocation API exists precisely so this compiles: every
        // borrow below is live at the final asserts.
        let arena = Arena::new();
        let a = arena.alloc_str("alpha").unwrap();
        let b = arena.alloc_str("beta").unwrap();
        let c = arena.alloc_slice(&[1u64, 2, 3]).unwrap();
        assert_eq!(a, "alpha");
        assert_eq!(b, "beta");
        assert_eq!(c, &[1u64, 2, 3]);
    }

    #[test]
    fn arena_alloc_str_copies_content() {
        let arena = Arena::new();
        let original = "hello".to_string();
        let borrowed = arena.alloc_str(&original).unwrap();
        assert_eq!(borrowed, "hello");
        // The arena holds its own copy; the source can be dropped or mutated.
        drop(original);
        assert_eq!(borrowed, "hello");
    }

    #[test]
    fn arena_alloc_slice_respects_alignment() {
        #[repr(align(16))]
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        struct Aligned16(u64, u64);

        let arena = Arena::with_chunk_size(64);
        // Misalign the cursor first with an odd-sized byte allocation.
        let _ = arena.alloc_bytes(3).unwrap();
        let slice = arena
            .alloc_slice(&[Aligned16(1, 2), Aligned16(3, 4)])
            .unwrap();
        assert_eq!(slice.as_ptr() as usize % 16, 0);
        assert_eq!(slice, &[Aligned16(1, 2), Aligned16(3, 4)]);

        let _ = arena.alloc_bytes(1).unwrap();
        let nums = arena.alloc_slice(&[1u64, 2u64]).unwrap();
        assert_eq!(nums.as_ptr() as usize % 8, 0);
        assert_eq!(nums, &[1u64, 2u64]);
    }

    #[test]
    fn arena_grows_when_chunk_full() {
        let arena = Arena::with_chunk_size(64);
        let first = arena.alloc_bytes(64).unwrap(); // fills the first chunk
        first[0] = 0xAB;
        let second = arena.alloc_bytes(64).unwrap(); // forces a new chunk
        second[0] = 0xCD;
        assert!(arena.capacity_bytes() >= 128);
        assert_eq!(first[0], 0xAB); // earlier chunk is intact
        assert_eq!(second[0], 0xCD);
        assert_eq!(arena.used_bytes(), 128);
    }

    #[test]
    fn arena_oversized_single_alloc_gets_dedicated_chunk() {
        let arena = Arena::with_chunk_size(64);
        let big = arena.alloc_bytes(10_000).unwrap();
        assert_eq!(big.len(), 10_000);
        big[9_999] = 0xFF;
        assert_eq!(big[9_999], 0xFF);
        assert!(arena.capacity_bytes() >= 10_000);
    }

    #[test]
    fn arena_used_bytes_tracks_bump_cursor() {
        let arena = Arena::new();
        assert_eq!(arena.used_bytes(), 0);
        assert_eq!(arena.capacity_bytes(), arena.chunk_size());
        let _ = arena.alloc_bytes(10).unwrap();
        assert_eq!(arena.used_bytes(), 10);
        let _ = arena.alloc_bytes(6).unwrap();
        assert_eq!(arena.used_bytes(), 16);
    }

    #[test]
    fn arena_reset_rewinds_and_reuses_first_chunk() {
        let mut arena = Arena::with_chunk_size(64);
        let _ = arena.alloc_bytes(200).unwrap(); // forces growth
        let grown_capacity = arena.capacity_bytes();
        assert!(grown_capacity > 64);

        arena.reset();
        assert_eq!(arena.used_bytes(), 0);
        assert_eq!(arena.capacity_bytes(), 64); // only the first chunk is kept

        let s = arena.alloc_str("reused").unwrap();
        assert_eq!(s, "reused");
        assert_eq!(arena.used_bytes(), 6);
    }

    #[test]
    fn arena_zero_length_allocs() {
        let arena = Arena::new();
        let bytes = arena.alloc_bytes(0).unwrap();
        assert!(bytes.is_empty());
        let slice = arena.alloc_slice::<u64>(&[]).unwrap();
        assert!(slice.is_empty());
        let s = arena.alloc_str("").unwrap();
        assert_eq!(s, "");
        assert_eq!(arena.used_bytes(), 0);
    }

    /// Regression: `alloc_bytes` hands out `&mut [u8]`, which safe code may
    /// read. After `reset` the first chunk is reused, and its bytes may hold
    /// the uninitialized padding of earlier typed allocations — so the bytes
    /// must be (re)initialized, not merely "fresh".
    #[test]
    fn arena_alloc_bytes_is_zeroed_after_reset() {
        let mut arena = Arena::new();
        arena.alloc_bytes(64).unwrap().fill(0xAB);
        arena.reset();
        assert!(arena.alloc_bytes(64).unwrap().iter().all(|&b| b == 0));
    }

    /// Regression: a zero-size request must still yield a pointer aligned for
    /// the requested alignment — callers cast it to `*mut T` for zero-sized
    /// `T` (e.g. `[u64; 0]`), and a misaligned pointer is UB for
    /// `copy_nonoverlapping` / `slice::from_raw_parts` / `ptr::write`.
    #[test]
    fn arena_zero_size_raw_alloc_is_aligned() {
        let arena = Arena::new();
        for align in [1usize, 2, 4, 8, 16, 64, 4096] {
            let ptr = arena.alloc_raw(0, align).unwrap();
            assert!(!ptr.is_null());
            assert_eq!(
                ptr.addr() % align,
                0,
                "zero-size alloc misaligned for {align}"
            );
        }
        assert_eq!(arena.used_bytes(), 0);
    }

    #[test]
    fn arena_zero_sized_type_allocs_are_aligned() {
        #[derive(Clone, Copy, Debug, PartialEq)]
        #[repr(align(64))]
        struct Aligned64;

        let arena = Arena::new();
        let slice = arena.alloc_slice(&[[0u64; 0]; 3]).unwrap();
        assert_eq!(slice.len(), 3);
        assert_eq!(slice.as_ptr().addr() % core::mem::align_of::<[u64; 0]>(), 0);

        let mutable = arena.alloc_slice_mut(5, Aligned64).unwrap();
        assert_eq!(mutable.len(), 5);
        assert_eq!(mutable.as_ptr().addr() % 64, 0);

        let value: &mut Aligned64 = arena.alloc_value(Aligned64).unwrap();
        assert_eq!((value as *const Aligned64).addr() % 64, 0);
        assert_eq!(*value, Aligned64);
        assert_eq!(arena.used_bytes(), 0);
    }

    /// Regression for the aliasing model (run under Miri:
    /// `cargo +nightly miri test -p relvar-core --lib values::arena`).
    /// Earlier `&mut` allocations must stay usable while later allocations
    /// carve from the same chunk, grow new chunks, and reallocate the chunk
    /// list. Allocating must never create a reference over chunk memory that
    /// was already handed out, nor write through a pointer derived from a
    /// shared borrow.
    #[test]
    fn arena_earlier_mut_borrows_stay_valid_across_later_allocations() {
        let arena = Arena::with_chunk_size(16);
        let first = arena.alloc_bytes(4).unwrap();
        let second = arena.alloc_slice_mut(2, 0u32).unwrap();
        // Same chunk, then several new chunks (the chunk list reallocates).
        let mut later = Vec::new();
        for i in 0..8u8 {
            let bytes = arena.alloc_bytes(12).unwrap();
            bytes.fill(i);
            later.push(bytes);
        }
        first.copy_from_slice(&[1, 2, 3, 4]);
        second[1] = 7;
        assert_eq!(first, &[1u8, 2, 3, 4]);
        assert_eq!(second, &[0u32, 7]);
        for (i, bytes) in later.iter().enumerate() {
            assert!(bytes.iter().all(|&b| usize::from(b) == i));
        }
    }

    #[test]
    fn arena_zero_size_raw_alloc_rejects_invalid_alignment() {
        let arena = Arena::new();
        assert!(arena.alloc_raw(0, 3).is_err());
        assert!(arena.alloc_raw(0, 0).is_err());
    }

    #[test]
    fn arena_size_overflow_errors() {
        let arena = Arena::new();
        let err = arena.alloc_bytes(usize::MAX).unwrap_err();
        assert_eq!(err, AllocError::SizeOverflow(usize::MAX));
    }

    #[test]
    fn arena_default_chunk_size() {
        let arena = Arena::default();
        assert_eq!(arena.chunk_size(), 4096);
        assert_eq!(arena.capacity_bytes(), 4096);
    }

    // -- ScalarRef -------------------------------------------------------

    #[test]
    fn scalar_ref_roundtrip_primitives() {
        let arena = Arena::new();
        for owned in [
            ScalarValue::Int(-42),
            ScalarValue::Int(i64::MIN),
            ScalarValue::Float(2.5),
            ScalarValue::Float(-0.0),
            ScalarValue::Bool(true),
            ScalarValue::Bool(false),
        ] {
            let borrowed = ScalarRef::from_owned(&owned, &arena).unwrap();
            let back = borrowed.to_owned().unwrap();
            assert_eq!(back, owned);
        }
    }

    #[test]
    fn scalar_ref_nan_equals_nan_for_set_semantics() {
        let arena = Arena::new();
        let nan = ScalarValue::Float(f64::NAN);
        let borrowed = ScalarRef::from_owned(&nan, &arena).unwrap();
        // Bit equality: NaN == NaN, matching ScalarValue.
        assert_eq!(borrowed, ScalarRef::Float(f64::NAN));
        assert_eq!(borrowed, borrowed);

        // Hash consistency: equal values hash equally, so set dedup works.
        let mut set = HashSet::new();
        set.insert(borrowed);
        assert!(set.contains(&ScalarRef::Float(f64::NAN)));
        assert_eq!(hash_of(&borrowed), hash_of(&ScalarRef::Float(f64::NAN)));
    }

    #[test]
    fn scalar_ref_text_binary_roundtrip() {
        let arena = Arena::new();
        let text = ScalarValue::String("Alice".to_string());
        let borrowed = ScalarRef::from_owned(&text, &arena).unwrap();
        assert_eq!(borrowed, ScalarRef::Text("Alice"));
        assert_eq!(borrowed.to_owned().unwrap(), text);

        let binary = ScalarValue::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let borrowed = ScalarRef::from_owned(&binary, &arena).unwrap();
        assert_eq!(borrowed, ScalarRef::Binary(&[0xDE, 0xAD, 0xBE, 0xEF]));
        assert_eq!(borrowed.to_owned().unwrap(), binary);
    }

    #[test]
    fn scalar_ref_user_defined_roundtrip() {
        let widget_id = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let owned = ScalarValue::select(&widget_id, ScalarValue::Int(7)).unwrap();

        let arena = Arena::new();
        let borrowed = ScalarRef::from_owned(&owned, &arena).unwrap();
        assert!(matches!(borrowed, ScalarRef::UserDefined { .. }));

        let back = borrowed.to_owned().unwrap();
        assert_eq!(back, owned);
    }

    #[test]
    fn scalar_ref_nested_relation_roundtrip() {
        let item_heading = TupleType::new()
            .with_attribute("product_id", ScalarType::Int)
            .with_attribute("quantity", ScalarType::Int);
        let mut items = Relation::new(RelationType::new(item_heading.clone()));
        items
            .insert(tuple! { product_id: 7i64, quantity: 3i64 })
            .unwrap();
        items
            .insert(tuple! { product_id: 8i64, quantity: 1i64 })
            .unwrap();

        let order_heading = TupleType::new()
            .with_attribute("order_id", ScalarType::Int)
            .with_attribute(
                "items",
                ScalarType::Relation(Box::new(RelationType::new(item_heading))),
            );
        let order = Tuple::new(
            order_heading,
            [
                ("order_id".to_string(), ScalarValue::Int(42)),
                ("items".to_string(), ScalarValue::Relation(items)),
            ],
        )
        .unwrap();

        let arena = Arena::new();
        let view = TupleView::from_tuple(&order, &arena).unwrap();
        assert!(view.conforms());
        assert!(matches!(view.get("items"), Some(ScalarRef::Relation(_))));

        // Full round-trip preserves the nested relation.
        let back = view.to_tuple().unwrap();
        assert_eq!(back, order);
    }

    // -- TupleView --------------------------------------------------------

    #[test]
    fn tuple_view_from_tuple_roundtrip() {
        let arena = Arena::new();
        let tuple = tuple! { id: 1i64, name: "Alice", active: true };
        let view = TupleView::from_tuple(&tuple, &arena).unwrap();
        assert!(view.conforms());
        assert_eq!(view.len(), 3);
        assert!(!view.is_empty());
        assert_eq!(view.heading.degree(), 3);

        let back = view.to_tuple().unwrap();
        assert_eq!(back, tuple);
    }

    #[test]
    fn tuple_view_get_by_name() {
        let arena = Arena::new();
        let tuple = tuple! { id: 1i64, name: "Alice", active: true };
        let view = TupleView::from_tuple(&tuple, &arena).unwrap();
        assert_eq!(view.get("id"), Some(ScalarRef::Int(1)));
        assert_eq!(view.get("name"), Some(ScalarRef::Text("Alice")));
        assert_eq!(view.get("active"), Some(ScalarRef::Bool(true)));
        assert_eq!(view.get("missing"), None);
    }

    #[test]
    fn tuple_view_conforms_checks_arity() {
        let heading = test_heading();
        let arena = Arena::new();

        // A hand-built view violating the positional contract does not conform.
        // Note: "active" sorts first in attribute-name order.
        let short = arena.alloc_slice(&[ScalarRef::Int(1)]).unwrap();
        let bad = TupleView {
            heading: &heading,
            values: short,
        };
        assert!(!bad.conforms());
        assert_eq!(bad.get("active"), Some(ScalarRef::Int(1)));
        assert_eq!(bad.get("id"), None);
    }

    #[test]
    fn tuple_view_equality_is_order_independent() {
        let arena = Arena::new();
        let ta = tuple! { id: 1i64, name: "Alice" };
        let tb = tuple! { id: 1i64, name: "Alice" };
        let tc = tuple! { id: 2i64, name: "Alice" };
        let a = TupleView::from_tuple(&ta, &arena).unwrap();
        let b = TupleView::from_tuple(&tb, &arena).unwrap();
        let c = TupleView::from_tuple(&tc, &arena).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(hash_of(&a), hash_of(&b));
    }

    // -- SlotRelation -----------------------------------------------------

    #[test]
    fn slot_relation_insert_until_full_errors() {
        let heading = test_heading();
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 2);
        assert!(slots.is_empty());
        assert_eq!(slots.len(), 0);
        assert_eq!(slots.capacity(), 2);

        let t1 = tuple! { id: 1i64, name: "Alice", active: true };
        let t2 = tuple! { id: 2i64, name: "Bob", active: false };
        let t3 = tuple! { id: 3i64, name: "Carol", active: true };
        let v1 = TupleView::from_tuple(&t1, &arena).unwrap();
        slots.insert(v1.values).unwrap();
        let v2 = TupleView::from_tuple(&t2, &arena).unwrap();
        slots.insert(v2.values).unwrap();
        assert_eq!(slots.len(), 2);

        // Full: CapacityError, not a panic.
        let v3 = TupleView::from_tuple(&t3, &arena).unwrap();
        let err = slots.insert(v3.values).unwrap_err();
        assert!(matches!(err, CapacityError::Full { capacity: 2 }));
        assert_eq!(slots.len(), 2);
    }

    // -- SlotRelation: set semantics and heading conformance ---------------
    //
    // TTM Proscription 2 (no duplicate tuples) and RM Prescription 9/10
    // (every tuple conforms to the heading) hold for slot relations too.

    fn int_heading() -> TupleType {
        TupleType::new().with_attribute("n", ScalarType::Int)
    }

    #[test]
    fn slot_relation_insert_duplicate_is_noop() {
        let heading = int_heading();
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 4);
        slots.insert(&[ScalarRef::Int(1)]).unwrap();
        slots.insert(&[ScalarRef::Int(1)]).unwrap();
        assert_eq!(slots.len(), 1);
    }

    #[test]
    fn slot_relation_insert_duplicate_into_full_relation_is_noop() {
        let heading = int_heading();
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 1);
        slots.insert(&[ScalarRef::Int(1)]).unwrap();
        // Already present: a no-op, not a capacity error.
        assert!(slots.insert(&[ScalarRef::Int(1)]).is_ok());
        assert_eq!(slots.len(), 1);
    }

    #[test]
    fn slot_relation_insert_reports_whether_tuple_was_new() {
        let heading = int_heading();
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 4);
        assert!(slots.insert(&[ScalarRef::Int(1)]).unwrap());
        assert!(!slots.insert(&[ScalarRef::Int(1)]).unwrap());
        assert!(slots.insert(&[ScalarRef::Int(2)]).unwrap());
    }

    #[test]
    fn slot_relation_insert_dedups_nan_and_keeps_signed_zeros_apart() {
        let heading = TupleType::new().with_attribute("f", ScalarType::Float);
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 8);
        slots.insert(&[ScalarRef::Float(f64::NAN)]).unwrap();
        slots
            .insert(&[ScalarRef::Float(f64::from_bits(f64::NAN.to_bits() ^ 1))])
            .unwrap();
        slots.insert(&[ScalarRef::Float(0.0)]).unwrap();
        slots.insert(&[ScalarRef::Float(-0.0)]).unwrap();
        assert_eq!(slots.len(), 3);
    }

    #[test]
    fn slot_relation_many_inserts_stay_deduplicated() {
        let heading = int_heading();
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 200);
        for round in 0..2 {
            for n in 0..150i64 {
                slots.insert(&[ScalarRef::Int(n)]).unwrap();
            }
            assert_eq!(slots.len(), 150, "round {round}");
        }
        let probe = SlotRelation::with_capacity(&arena, &heading, 0);
        assert_ne!(slots, probe);
        assert_eq!(slots.to_relation().unwrap().cardinality(), 150);
    }

    #[test]
    fn slot_relation_dedups_against_from_relation_contents() {
        let owned = test_relation();
        let arena = Arena::new();
        let mut slots = SlotRelation::from_relation(&owned, &arena, 8).unwrap();
        let alice = tuple! { id: 1i64, name: "Alice", active: true };
        let view = TupleView::from_tuple(&alice, &arena).unwrap();
        slots.insert(view.values()).unwrap();
        assert_eq!(slots.len(), 2);
    }

    #[test]
    fn slot_relation_clear_resets_deduplication() {
        let heading = int_heading();
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 2);
        slots.insert(&[ScalarRef::Int(1)]).unwrap();
        slots.clear();
        assert!(slots.insert(&[ScalarRef::Int(1)]).unwrap());
        assert_eq!(slots.len(), 1);
    }

    #[test]
    fn slot_relation_insert_rejects_value_of_wrong_type() {
        let heading = int_heading();
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 4);
        assert!(slots.insert(&[ScalarRef::Text("one")]).is_err());
        assert!(slots.is_empty());
    }

    #[test]
    fn slot_relation_insert_reports_mismatched_position() {
        let heading = test_heading(); // active: Bool, id: Int, name: String
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 4);
        let err = slots
            .insert(&[ScalarRef::Bool(true), ScalarRef::Int(1), ScalarRef::Int(2)])
            .unwrap_err();
        assert!(matches!(err, CapacityError::TypeMismatch { position: 2 }));
    }

    #[test]
    fn slot_relation_insert_rejects_user_defined_value_of_other_type() {
        let widget = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let supplier = ScalarType::user_defined("SupplierId", ScalarType::Int);
        let heading = TupleType::new().with_attribute("w", widget.clone());
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 4);

        let one = ScalarRef::Int(1);
        let as_supplier = ScalarRef::UserDefined {
            type_def: &supplier,
            value: &one,
        };
        assert!(slots.insert(&[as_supplier]).is_err());
        // The bare representation is not a WidgetId either.
        assert!(slots.insert(&[one]).is_err());
        // A WidgetId whose representation is not an Int is malformed.
        let text = ScalarRef::Text("1");
        let malformed = ScalarRef::UserDefined {
            type_def: &widget,
            value: &text,
        };
        assert!(slots.insert(&[malformed]).is_err());

        let as_widget = ScalarRef::UserDefined {
            type_def: &widget,
            value: &one,
        };
        assert!(slots.insert(&[as_widget]).unwrap());
    }

    #[test]
    fn slot_relation_insert_rejects_nested_relation_of_other_heading() {
        let inner = TupleType::new().with_attribute("x", ScalarType::Int);
        let other_inner = TupleType::new().with_attribute("y", ScalarType::Int);
        let heading = TupleType::new().with_attribute(
            "r",
            ScalarType::Relation(Box::new(RelationType::new(inner.clone()))),
        );
        let arena = Arena::new();
        let good = SlotRelation::with_capacity(&arena, &inner, 0);
        let bad = SlotRelation::with_capacity(&arena, &other_inner, 0);
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 4);
        assert!(slots.insert(&[ScalarRef::Relation(&bad)]).is_err());
        assert!(slots.insert(&[ScalarRef::Relation(&good)]).unwrap());
    }

    #[test]
    fn slot_relation_equality_is_symmetric_after_repeated_insert() {
        let heading = int_heading();
        let arena = Arena::new();
        let mut aa = SlotRelation::with_capacity(&arena, &heading, 4);
        aa.insert(&[ScalarRef::Int(1)]).unwrap();
        aa.insert(&[ScalarRef::Int(1)]).unwrap();
        let mut ab = SlotRelation::with_capacity(&arena, &heading, 4);
        ab.insert(&[ScalarRef::Int(1)]).unwrap();
        ab.insert(&[ScalarRef::Int(2)]).unwrap();
        assert_ne!(aa, ab);
        assert_ne!(ab, aa);
    }

    #[test]
    fn slot_relation_hash_is_coherent_with_eq_after_repeated_insert() {
        let heading = int_heading();
        let arena = Arena::new();
        let mut twice = SlotRelation::with_capacity(&arena, &heading, 4);
        twice.insert(&[ScalarRef::Int(1)]).unwrap();
        twice.insert(&[ScalarRef::Int(1)]).unwrap();
        let mut once = SlotRelation::with_capacity(&arena, &heading, 4);
        once.insert(&[ScalarRef::Int(1)]).unwrap();
        assert_eq!(twice, once);
        assert_eq!(hash_of(&twice), hash_of(&once));
        // XOR of duplicate hashes must not cancel out to the empty relation.
        let empty = SlotRelation::with_capacity(&arena, &heading, 4);
        let mut pair = SlotRelation::with_capacity(&arena, &heading, 4);
        pair.insert(&[ScalarRef::Int(5)]).unwrap();
        pair.insert(&[ScalarRef::Int(5)]).unwrap();
        assert_ne!(pair, empty);
        assert_ne!(hash_of(&pair), hash_of(&empty));
    }

    // -- TupleView: validated construction --------------------------------

    #[test]
    fn tuple_view_new_accepts_conforming_values() {
        let heading = int_heading();
        let values = [ScalarRef::Int(3)];
        let view = TupleView::new(&heading, &values).unwrap();
        assert!(view.conforms());
        assert_eq!(view.get("n"), Some(ScalarRef::Int(3)));
        assert!(core::ptr::eq(view.heading(), &heading));
        assert_eq!(view.values(), &values);
    }

    #[test]
    fn tuple_view_new_rejects_wrong_arity_and_types() {
        let heading = int_heading();
        assert!(matches!(
            TupleView::new(&heading, &[]),
            Err(CapacityError::ArityMismatch {
                expected: 1,
                found: 0
            })
        ));
        assert!(matches!(
            TupleView::new(&heading, &[ScalarRef::Bool(true)]),
            Err(CapacityError::TypeMismatch { position: 0 })
        ));
    }

    #[test]
    fn slot_relation_insert_arity_mismatch_errors() {
        let heading = test_heading();
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 8);
        let err = slots.insert(&[ScalarRef::Int(1)]).unwrap_err();
        assert!(matches!(
            err,
            CapacityError::ArityMismatch {
                expected: 3,
                found: 1
            }
        ));
        assert!(slots.is_empty());
    }

    #[test]
    fn slot_relation_from_relation_roundtrip() {
        let owned = test_relation();
        let arena = Arena::new();
        let slots = SlotRelation::from_relation(&owned, &arena, 8).unwrap();
        assert_eq!(slots.len(), 2);
        assert_eq!(slots.heading().degree(), 3);
        assert!(!slots.is_empty());

        let back = slots.to_relation().unwrap();
        assert_eq!(back, owned);
    }

    #[test]
    fn slot_relation_from_relation_capacity_too_small() {
        let owned = test_relation();
        let arena = Arena::new();
        let err = SlotRelation::from_relation(&owned, &arena, 1).unwrap_err();
        assert!(matches!(
            err,
            SlotRelationError::Capacity(CapacityError::Full { capacity: 1 })
        ));
    }

    #[test]
    fn slot_relation_clear_keeps_capacity() {
        let heading = test_heading();
        let arena = Arena::new();
        let mut slots = SlotRelation::with_capacity(&arena, &heading, 2);
        let t1 = tuple! { id: 1i64, name: "Alice", active: true };
        let v1 = TupleView::from_tuple(&t1, &arena).unwrap();
        slots.insert(v1.values).unwrap();
        assert_eq!(slots.len(), 1);

        slots.clear();
        assert!(slots.is_empty());
        assert_eq!(slots.capacity(), 2);

        // Usable again after clear.
        slots.insert(v1.values).unwrap();
        assert_eq!(slots.len(), 1);
    }

    #[test]
    fn slot_relation_equality_ignores_slot_order() {
        // `Relation::tuples` iterates a HashSet (no defined order), so two
        // conversions may lay slots out differently; equality is set-based.
        let owned = test_relation();
        let arena = Arena::new();
        let a = SlotRelation::from_relation(&owned, &arena, 8).unwrap();
        let b = SlotRelation::from_relation(&owned, &arena, 8).unwrap();
        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));

        let other = test_relation();
        let arena2 = Arena::new();
        let mut c = SlotRelation::from_relation(&other, &arena2, 8).unwrap();
        c.clear();
        assert_ne!(a, c);
    }

    #[test]
    fn slot_relation_iter_yields_views() {
        let owned = test_relation();
        let arena = Arena::new();
        let slots = SlotRelation::from_relation(&owned, &arena, 8).unwrap();
        let mut ids: Vec<i64> = slots
            .iter()
            .map(|view| match view.get("id") {
                Some(ScalarRef::Int(id)) => id,
                other => panic!("expected Int id, got {other:?}"),
            })
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2]);

        let first = slots.iter().next().unwrap();
        assert!(slots.contains(&first));
    }

    // -- Crate error integration ------------------------------------------

    #[test]
    fn alloc_error_integrates_with_database_error() {
        let err: crate::error::DatabaseError = AllocError::SizeOverflow(1).into();
        assert!(matches!(err, crate::error::DatabaseError::AllocError(_)));
        assert_eq!(
            err.to_string(),
            "Arena allocation failed: allocation size overflow: 1"
        );
    }
}
