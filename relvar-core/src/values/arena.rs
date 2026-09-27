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
//! - [`SlotRelation`] is a fixed-capacity slot array of tuple views. The arena
//!   and the heading are caller-owned borrows (`&'a Arena`, `&'a TupleType`),
//!   so no raw pointers or `unsafe` appear outside [`Arena`] itself.
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
//! - **Proscription 2** (no duplicates): [`SlotRelation`] is a *physical*
//!   slot array and does not deduplicate on insert — duplicate elimination is
//!   the algebra layer's job. [`SlotRelation::from_relation`] sources are sets
//!   by construction, and [`SlotRelation::to_relation`] materializes through
//!   [`Relation`]'s set semantics.
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

/// Errors from inserting a tuple into a [`SlotRelation`].
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
/// These only fire when a view violates its construction contract (for
/// example, a hand-built [`TupleView`] whose values do not match its heading);
/// views produced by `from_tuple` / `insert` / `from_relation` convert
/// infallibly in practice.
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
/// The backing buffer never moves after creation (it is heap-allocated and
/// only the `Vec` of chunks itself may reallocate, which moves the `Box`, not
/// the bytes), so raw pointers into it stay valid for the arena's lifetime.
#[derive(Debug)]
struct Chunk {
    storage: Box<[u8]>,
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
/// The single `unsafe` block in this module (in [`Arena::alloc_bytes`])
/// upholds these invariants:
///
/// - Allocations are carved from a monotonically increasing cursor, so no two
///   live allocations overlap.
/// - Chunk byte buffers are heap-stable: pushing a new chunk may reallocate
///   the chunk `Vec`, but that moves the `Box`es, never the bytes they own.
/// - `reset` and `drop` require `&mut self`; the returned references are tied
///   to the `&self` borrow in the method signatures, so the borrow checker
///   statically prevents invalidation while borrows are live.
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
        let storage: Box<[u8]> = alloc::vec![0u8; chunk_size].into_boxed_slice();
        Self {
            chunks: RefCell::new(alloc::vec![Chunk { storage }]),
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
                    chunk.storage.len()
                }
            })
            .sum()
    }

    /// Total bytes reserved across all chunks.
    pub fn capacity_bytes(&self) -> usize {
        self.chunks
            .borrow()
            .iter()
            .map(|chunk| chunk.storage.len())
            .sum()
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

    /// Allocates `n` uninitialized bytes and returns them as a mutable slice.
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
        // SAFETY: `ptr` addresses `n` bytes of fresh, disjoint arena memory.
        // The chunk outlives the `&self` borrow this reference is tied to:
        // chunk byte buffers never move, and `reset`/`drop` require `&mut
        // self`, which the borrow checker forbids while this borrow is live.
        Ok(unsafe { core::slice::from_raw_parts_mut(ptr, n) })
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
        // SAFETY: `bytes` is an exact copy of `s`, which is valid UTF-8.
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
        // SAFETY: `ptr` addresses `items.len()` properly-aligned slots of
        // fresh, disjoint arena memory; `T: Copy` makes bitwise copy valid; each
        // slot is written exactly once before the shared slice is formed.
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
        // SAFETY: `ptr` addresses `len` properly-aligned slots of fresh,
        // disjoint arena memory; every slot is written exactly once (via
        // `fill`, valid for `T: Copy`) before the exclusive slice is formed.
        // Same invariants as `alloc_slice`, plus exclusive access.
        let slice = unsafe { core::slice::from_raw_parts_mut(ptr, len) };
        slice.fill(value);
        Ok(slice)
    }

    /// Moves `value` into arena memory and returns an exclusive borrow of it.
    ///
    /// Used to place non-`Copy` structures (nested [`SlotRelation`]s, nested
    /// [`ScalarRef`]s, cloned [`ScalarType`]s) inside the arena during
    /// [`ScalarRef::from_owned`] conversion.
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
        // SAFETY: `ptr` addresses one properly-aligned slot of fresh arena
        // memory; it is written exactly once here, before the exclusive borrow
        // is formed. (For zero-sized `T` the pointer is dangling but no memory
        // is touched, which is valid.)
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
        if size == 0 {
            return Ok(core::ptr::NonNull::dangling().as_ptr());
        }
        // Validates that `align` is a non-zero power of two and that `size`
        // fits the platform's layout rules; the manual arithmetic below relies
        // on both.
        let _ = core::alloc::Layout::from_size_align(size, align)
            .map_err(|_| AllocError::SizeOverflow(size))?;

        // Fast path: room in the current (last) chunk.
        let reused = {
            let chunks = self.chunks.borrow();
            match chunks.last() {
                Some(chunk) => {
                    let base = chunk.storage.as_ptr() as usize;
                    let cursor = self.cursor.get();
                    // Align the absolute address, not just the cursor.
                    let placed = base.checked_add(cursor).and_then(|addr| {
                        let aligned = addr.checked_add(align - 1)? & !(align - 1);
                        let end = aligned.checked_add(size)?;
                        (end <= base + chunk.storage.len()).then_some((aligned, end - base))
                    });
                    if let Some((aligned, new_cursor)) = placed {
                        self.cursor.set(new_cursor);
                        Some(aligned as *mut u8)
                    } else {
                        None
                    }
                }
                None => None,
            }
        };
        if let Some(ptr) = reused {
            return Ok(ptr);
        }

        // Slow path: the current chunk is full (or missing); grow.
        // Worst-case padding is `align - 1`, so `size + align` always fits.
        let need = size
            .checked_add(align)
            .ok_or(AllocError::SizeOverflow(size))?;
        let chunk_len = self.chunk_size.max(need);
        let storage: Box<[u8]> = alloc::vec![0u8; chunk_len].into_boxed_slice();
        let base = storage.as_ptr() as usize;
        self.chunks.borrow_mut().push(Chunk { storage });
        // The new chunk is empty: align its base address. `chunk_len >= size +
        // align` guarantees the allocation fits.
        let aligned = base
            .checked_add(align - 1)
            .ok_or(AllocError::SizeOverflow(size))?
            & !(align - 1);
        self.cursor.set(aligned - base + size);
        Ok(aligned as *mut u8)
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
/// `values` is positional in **heading attribute order** (the order yielded by
/// [`TupleType::attribute_names`]): `values[i]` is the value of the `i`-th
/// attribute. [`TupleView::from_tuple`] and [`SlotRelation::insert`] both
/// establish this layout; hand-built views must uphold it or [`TupleView::get`]
/// will misattribute values.
///
/// `Copy` (like `&` references): views are cheap to pass around; the data
/// lives in the arena.
#[derive(Clone, Copy, Debug)]
pub struct TupleView<'a> {
    /// The borrowed heading this view conforms to.
    pub heading: &'a TupleType,
    /// The attribute values, positional in heading attribute order.
    pub values: &'a [ScalarRef<'a>],
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

    /// Lightweight structural check: the value count matches the heading
    /// degree.
    ///
    /// This is allocation-free. Full type conformance is established at the
    /// `from_tuple` / `insert` boundaries, which is where values enter the
    /// arena layer.
    pub fn conforms(&self) -> bool {
        self.values.len() == self.heading.degree()
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
/// `SlotRelation` is a *physical* container, not a logical set: [`insert`][Self::insert]
/// does not deduplicate (TTM Proscription 2 is enforced at the relation
/// boundaries — [`from_relation`][Self::from_relation] sources are sets, and
/// [`to_relation`][Self::to_relation] materializes through [`Relation`]'s set
/// semantics).
#[derive(Debug)]
pub struct SlotRelation<'a> {
    arena: &'a Arena,
    heading: &'a TupleType,
    slots: Vec<TupleView<'a>>,
    capacity: usize,
}

// Set semantics for equality (TTM Proscription 2): same heading, same
// cardinality, and every view of one side is present in the other. Slot order
// is physical and ignored.
impl<'a> PartialEq for SlotRelation<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.heading == other.heading
            && self.len() == other.len()
            && self.iter().all(|view| other.contains(&view))
    }
}

impl<'a> Eq for SlotRelation<'a> {}

// Order-independent hash mirroring Relation's: heading, cardinality, and the
// XOR of per-view hashes (XOR is commutative, so slot order does not matter).
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
            let mut hasher = crate::collections::new_hasher();
            view.hash(&mut hasher);
            combined ^= core::hash::Hasher::finish(&hasher);
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

    /// Whether a view equal to `view` is stored (linear scan).
    pub fn contains(&self, view: &TupleView<'a>) -> bool {
        self.slots.iter().any(|slot| slot == view)
    }

    /// Inserts a tuple's values as a new view.
    ///
    /// `values` is positional in **heading attribute order**
    /// (`values[i]` is the value of the `i`-th attribute yielded by
    /// [`TupleType::attribute_names`]). The slice is copied into the arena and
    /// the resulting view is pushed into the slot array — no per-insert heap
    /// allocation beyond the arena bump.
    ///
    /// # Errors
    ///
    /// - [`CapacityError::Full`] if the slot array is at capacity (not a panic).
    /// - [`CapacityError::ArityMismatch`] if `values.len()` does not equal the
    ///   heading degree.
    /// - [`CapacityError::Alloc`] if the arena allocation fails.
    pub fn insert(&mut self, values: &[ScalarRef<'a>]) -> Result<(), CapacityError> {
        if values.len() != self.heading.degree() {
            return Err(CapacityError::ArityMismatch {
                expected: self.heading.degree(),
                found: values.len(),
            });
        }
        if self.slots.len() >= self.capacity {
            return Err(CapacityError::Full {
                capacity: self.capacity,
            });
        }
        let stored = self.arena.alloc_slice(values)?;
        self.slots.push(TupleView {
            heading: self.heading,
            values: stored,
        });
        Ok(())
    }

    /// Removes all tuple views, keeping the capacity.
    ///
    /// The arena is *not* reset — arena memory stays occupied until the caller
    /// explicitly calls [`Arena::reset`].
    pub fn clear(&mut self) {
        self.slots.clear();
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
    /// This allocates (it leaves the zero-alloc layer). Duplicates — if any
    /// were inserted directly — collapse here through [`Relation`]'s set
    /// semantics.
    ///
    /// # Errors
    ///
    /// Returns [`MaterializeError`] if a view violates its construction
    /// contract. For relations built by `from_relation` / `insert` this is
    /// practically infallible.
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
