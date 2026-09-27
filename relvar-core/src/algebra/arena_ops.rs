//! Arena-native relational algebra operators (v0.8 "zero-alloc values").
//!
//! Zero-alloc counterparts to the owned operators in [`crate::algebra`]:
//! every operator consumes [`SlotRelation`] tuples and produces a new
//! [`SlotRelation`], with all value data living in the caller-owned
//! [`Arena`]. This module is **new, additive API** — the existing owned
//! operators are untouched.
//!
//! # Allocation discipline
//!
//! The per-tuple hot path of every operator performs **zero heap
//! allocations**: tuple value slices, hash-table buckets, and scratch buffers
//! are all carved from the caller's [`Arena`]. Deduplication and the join
//! build side use [`ArenaSet`] and [`ArenaMap`]: open-addressing tables with
//! linear probing whose buckets are a single arena slice — no per-entry
//! allocation (Waymaker-style caller-sized storage).
//!
//! Two per-call, `O(degree)`-bounded setup costs remain outside the arena and
//! are documented so callers can plan for them:
//!
//! - Result headings are [`TupleType`]s built with the owned type layer
//!   (`BTreeMap`/`String`); the finished heading is then placed in the arena.
//!   [`restrict_in`] and the set operators reuse the input heading and pay
//!   nothing here.
//! - The output [`SlotRelation`]'s slot storage is reserved up front at the
//!   documented capacity bound.
//!
//! Callers that need even the per-call setup to be allocation-free can size
//! their own outputs and drive the `*_into` cores directly — this is what the
//! zero-alloc proof in the test module does.
//!
//! # Operator semantics
//!
//! - [`restrict_in`] (σ): keeps tuples satisfying the predicate; set
//!   semantics — duplicates are eliminated.
//! - [`project_in`] (π): keeps the named attributes; duplicates are
//!   eliminated. Unknown attribute names are an error
//!   ([`ArenaOpError::UnknownAttribute`]), never a panic.
//! - [`union_in`], [`difference_in`], [`intersect_in`]: set operators; both
//!   inputs must share one identical heading
//!   ([`ArenaOpError::HeadingMismatch`]).
//! - [`join_in`]: natural hash join keyed on **all** commonly-named
//!   attributes (the same key semantics as
//!   [`Relation::join`](crate::values::Relation::join)). The combined heading
//!   is `a`'s attributes plus `b`'s non-common attributes; on a name
//!   collision the left relation's attribute (and its type) wins. With no
//!   common attributes this is the Cartesian product.
//! - [`extend_in`]: appends one computed attribute; the new attribute's type
//!   is inferred from the first tuple's computed value.
//!
//! # TTM notes
//!
//! - **Proscription 1** (no NULLs): predicates and compute functions see only
//!   complete tuples; there is no null representation to handle.
//! - **Proscription 2** (no duplicate tuples): every operator eliminates
//!   duplicates explicitly — [`SlotRelation`] itself never dedupes on insert.
//! - **Proscription 3/4** (no ordering, no attribute order): outputs are
//!   sets; attribute order inside a heading follows [`TupleType`]'s canonical
//!   (name-sorted) layout, which carries no semantic weight.
//!
//! # Example
//!
//! ```
//! use relvar_core::algebra::{join_in, restrict_in};
//! use relvar_core::types::{RelationType, ScalarType, TupleType};
//! use relvar_core::values::{Arena, Relation, ScalarRef, SlotRelation, TupleView};
//! use relvar_core::tuple;
//!
//! let emp_heading = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("dept_id", ScalarType::Int);
//! let mut employees = Relation::new(RelationType::new(emp_heading));
//! employees.insert(tuple! { emp_id: 1i64, dept_id: 10i64 }).unwrap();
//! employees.insert(tuple! { emp_id: 2i64, dept_id: 20i64 }).unwrap();
//!
//! let dept_heading = TupleType::new()
//!     .with_attribute("dept_id", ScalarType::Int)
//!     .with_attribute("dept_name", ScalarType::String);
//! let mut departments = Relation::new(RelationType::new(dept_heading));
//! departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();
//!
//! let arena = Arena::new();
//! let emp_slots = SlotRelation::from_relation(&employees, &arena, 8).unwrap();
//! let dept_slots = SlotRelation::from_relation(&departments, &arena, 8).unwrap();
//!
//! let joined = join_in(&emp_slots, &dept_slots, &arena).unwrap();
//! assert_eq!(joined.len(), 1);
//!
//! let engineering = restrict_in(
//!     &joined,
//!     &|view: TupleView| view.get("dept_name") == Some(ScalarRef::Text("Engineering")),
//!     &arena,
//! )
//! .unwrap();
//! assert_eq!(engineering.len(), 1);
//! ```

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use core::hash::{Hash, Hasher};

use thiserror::Error;

use crate::collections::new_hasher;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::arena::{AllocError, CapacityError};
use crate::values::{Arena, ScalarRef, SlotRelation, TupleView};

/// Errors from the arena-native algebra operators.
///
/// Failures fall into two families. **Resource exhaustion**
/// ([`ArenaOpError::AllocError`], [`ArenaOpError::CapacityError`]) means the
/// caller's arena (or output capacity) was too small — the operator is
/// otherwise correct and can be retried with more room. **Schema errors**
/// ([`ArenaOpError::HeadingMismatch`], [`ArenaOpError::UnknownAttribute`],
/// [`ArenaOpError::ArityMismatch`], [`ArenaOpError::AttributeExists`],
/// [`ArenaOpError::TypeMismatch`],
/// [`ArenaOpError::CannotInferType`]) mean the inputs did not satisfy the
/// operator's contract; retrying changes nothing.
#[derive(Debug, Error)]
pub enum ArenaOpError {
    /// An arena allocation backing the operator failed (out of memory).
    ///
    /// Pre-size the arena (`Arena::with_chunk_size`) and retry.
    #[error("arena allocation failed: {0}")]
    AllocError(#[from] AllocError),

    /// A slot-relation capacity limit was hit: either the output relation
    /// was full, or a value slice did not match its heading's degree.
    ///
    /// When driving the `*_into` cores with caller-sized outputs, size the
    /// output at the documented upper bound for the operator.
    #[error("slot relation capacity error: {0}")]
    CapacityError(#[from] CapacityError),

    /// The two inputs of a set operator did not share one identical heading.
    ///
    /// Set operators (`union_in`, `difference_in`, `intersect_in`) require
    /// union compatibility: identical attribute names *and* identical
    /// attribute types.
    #[error("relation headings do not match for set operation")]
    HeadingMismatch,

    /// An attribute name did not resolve against the input heading.
    ///
    /// Returned by [`project_in`] (and by `*_into` cores handed a mismatched
    /// output heading) instead of panicking.
    #[error("unknown attribute: {0}")]
    UnknownAttribute(String),

    /// A value slice did not match the expected heading degree.
    ///
    /// Fired defensively by the `*_into` cores when a caller-sized output's
    /// heading does not match the core's contract (the public `*_in`
    /// wrappers build their headings themselves, so this only triggers on
    /// direct core misuse).
    #[error("arity mismatch: expected {expected} values but found {found}")]
    ArityMismatch {
        /// The value count the heading contract required.
        expected: usize,
        /// The value count actually found.
        found: usize,
    },

    /// A computed value's type did not match the new attribute's inferred
    /// type (extend's per-tuple check, mirroring the owned
    /// [`ExtendError::TupleCreation`](crate::algebra::ExtendError::TupleCreation)
    /// type check against the declared attribute type).
    #[error("type mismatch for attribute '{attribute}': expected {expected}, found {found}")]
    TypeMismatch {
        /// The new attribute's name.
        attribute: String,
        /// The inferred type's name.
        expected: String,
        /// The offending computed value's type name.
        found: String,
    },

    /// `extend_in` would overwrite an existing attribute of the same name.
    #[error("attribute '{0}' already exists")]
    AttributeExists(String),

    /// `extend_in` could not infer the new attribute's type because the input
    /// relation is empty (there is no tuple to compute the type from).
    #[error("cannot infer type of new attribute '{0}': input relation is empty")]
    CannotInferType(String),
}

// ---------------------------------------------------------------------------
// Arena-resident hash tables
//
// `ArenaSet` and `ArenaMap` are open-addressing tables with linear probing
// whose buckets are a single arena slice. All probing is index arithmetic;
// nothing is heap-allocated after the one arena slice. This is the
// Waymaker-style caller-sized storage the task calls for: the caller (or the
// public `*_in` wrapper) picks the capacity from a sound upper bound on the
// entry count, and every entry is a `Copy` [`TupleView`] — no hashing
// indirection, no per-entry allocation.
// ---------------------------------------------------------------------------

/// Hashes one tuple: the [`Hash`] impl of [`TupleView`] is order-independent,
/// so physically different but logically equal tuples hash identically.
fn tuple_hash(view: TupleView<'_>) -> u64 {
    let mut hasher = new_hasher();
    view.hash(&mut hasher);
    hasher.finish()
}

/// Rounds `entries` up to a bucket count with load factor ≤ 1/2 (power of
/// two, minimum 2). A zero-entry table still gets 2 buckets so probing is
/// well-defined.
fn bucket_count(entries: usize) -> usize {
    let mut buckets = 2usize;
    while buckets < entries.saturating_mul(2).max(2) {
        buckets = buckets.saturating_mul(2);
    }
    buckets.min(1 << 30)
}

/// One open-addressing bucket. The occupancy flag is folded into the
/// `Option`: `None` marks an empty bucket and terminates probing.
#[derive(Clone, Copy)]
struct Bucket<'a> {
    hash: u64,
    view: Option<TupleView<'a>>,
}

impl<'a> Bucket<'a> {
    /// An empty bucket.
    fn empty() -> Self {
        Self {
            hash: 0,
            view: None,
        }
    }
}

/// Arena-resident set of [`TupleView`]s for result deduplication.
///
/// Used by every operator that must enforce TTM Proscription 2 (no duplicate
/// tuples): the set operators, `project_in`, `restrict_in`, and the join
/// output. Buckets live in the arena; [`TupleView`] is `Copy`, so insertion
/// never allocates.
pub(crate) struct ArenaSet<'a> {
    buckets: &'a mut [Bucket<'a>],
    mask: usize,
}

impl<'a> ArenaSet<'a> {
    /// Creates an empty set sized for at most `capacity` entries.
    ///
    /// # Errors
    ///
    /// Returns [`ArenaOpError::AllocError`] if the arena cannot back the
    /// bucket slice.
    pub(crate) fn new(arena: &'a Arena, capacity: usize) -> Result<Self, ArenaOpError> {
        let buckets = bucket_count(capacity);
        Ok(Self {
            buckets: arena.alloc_slice_mut(buckets, Bucket::empty())?,
            mask: buckets - 1,
        })
    }

    /// Inserts `view`; returns `true` if it was newly present.
    ///
    /// Equality is [`TupleView`]'s order-independent tuple equality, so
    /// physically distinct but logically equal tuples collide by design and
    /// are reported as duplicates.
    pub(crate) fn insert(&mut self, view: TupleView<'a>) -> bool {
        let hash = tuple_hash(view);
        let mut index = (hash as usize) & self.mask;
        loop {
            let slot = &mut self.buckets[index];
            match slot.view {
                None => {
                    *slot = Bucket {
                        hash,
                        view: Some(view),
                    };
                    return true;
                }
                Some(existing) => {
                    if slot.hash == hash && existing == view {
                        return false;
                    }
                }
            }
            index = (index + 1) & self.mask;
        }
    }

    /// Returns `true` if `view` is already present.
    pub(crate) fn contains(&self, view: TupleView<'_>) -> bool {
        let hash = tuple_hash(view);
        let mut index = (hash as usize) & self.mask;
        loop {
            let slot = &self.buckets[index];
            match slot.view {
                None => return false,
                Some(existing) => {
                    if slot.hash == hash && existing == view {
                        return true;
                    }
                }
            }
            index = (index + 1) & self.mask;
        }
    }
}

/// Arena-resident multi-map from a 64-bit key hash to build-side tuples.
///
/// Used for the join build side. Keys are *hashes* of the common-attribute
/// projection; equality is re-checked with a caller-supplied closure, so a
/// 64-bit hash collision can never produce a wrong tuple — at worst one
/// extra comparison. Duplicate build-side keys are supported: several tuples
/// may share a probe chain.
pub(crate) struct ArenaMap<'a> {
    buckets: &'a mut [Bucket<'a>],
    mask: usize,
}

impl<'a> ArenaMap<'a> {
    /// Creates an empty map sized for at most `capacity` build entries.
    ///
    /// # Errors
    ///
    /// Returns [`ArenaOpError::AllocError`] if the arena cannot back the
    /// bucket slice.
    pub(crate) fn new(arena: &'a Arena, capacity: usize) -> Result<Self, ArenaOpError> {
        let buckets = bucket_count(capacity);
        Ok(Self {
            buckets: arena.alloc_slice_mut(buckets, Bucket::empty())?,
            mask: buckets - 1,
        })
    }

    /// Inserts one build-side tuple under its key `hash`.
    pub(crate) fn insert(&mut self, hash: u64, view: TupleView<'a>) {
        let mut index = (hash as usize) & self.mask;
        loop {
            let slot = &mut self.buckets[index];
            if slot.view.is_none() {
                *slot = Bucket {
                    hash,
                    view: Some(view),
                };
                return;
            }
            index = (index + 1) & self.mask;
        }
    }

    /// Calls `emit` for every build-side tuple whose key equals the probe
    /// tuple's key.
    ///
    /// `key_eq` compares a build-side tuple against the probe tuple on the
    /// actual key values (not just the hash).
    ///
    /// # Errors
    ///
    /// Propagates whatever `emit` returns.
    pub(crate) fn for_each_match<F>(
        &self,
        hash: u64,
        probe: TupleView<'a>,
        key_eq: impl Fn(TupleView<'a>, TupleView<'a>) -> bool,
        mut emit: F,
    ) -> Result<(), ArenaOpError>
    where
        F: FnMut(TupleView<'a>) -> Result<(), ArenaOpError>,
    {
        let mut index = (hash as usize) & self.mask;
        loop {
            let slot = &self.buckets[index];
            match slot.view {
                None => return Ok(()),
                Some(view) => {
                    if slot.hash == hash && key_eq(view, probe) {
                        emit(view)?;
                    }
                }
            }
            index = (index + 1) & self.mask;
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Appends one tuple to `out`, copying its value slice into the arena.
///
/// [`SlotRelation::insert`] already copies values into the arena; this
/// wrapper converts its [`CapacityError`] into [`ArenaOpError`] at one place.
fn append_tuple<'a>(
    out: &mut SlotRelation<'a>,
    values: &[ScalarRef<'a>],
) -> Result<(), ArenaOpError> {
    out.insert(values).map_err(ArenaOpError::from)
}

/// Returns `true` when two headings are union-compatible: identical attribute
/// names **and** identical attribute types.
fn headings_identical(a: &TupleType, b: &TupleType) -> bool {
    a.degree() == b.degree()
        && a.attributes()
            .iter()
            .all(|(name, scalar_type)| b.get_attribute_type(name) == Some(scalar_type))
}

/// Derives the [`ScalarType`] of a [`ScalarRef`] without touching the heap,
/// except for nested relation values (which need a boxed [`RelationType`])
/// and user-defined values (which clone their type descriptor).
///
/// Used once per `extend` call to build the new attribute's heading entry —
/// never on the per-tuple hot path.
fn scalar_type_of(value: ScalarRef<'_>) -> ScalarType {
    match value {
        ScalarRef::Int(_) => ScalarType::Int,
        ScalarRef::Float(_) => ScalarType::Float,
        ScalarRef::Bool(_) => ScalarType::Bool,
        ScalarRef::Text(_) => ScalarType::String,
        ScalarRef::Binary(_) => ScalarType::Bytes,
        ScalarRef::Relation(relation) => {
            ScalarType::Relation(Box::new(RelationType::new(relation.heading().clone())))
        }
        ScalarRef::UserDefined { type_def, .. } => (*type_def).clone(),
    }
}

/// Returns `true` when two [`ScalarRef`]s have compatible types for extend's
/// inferred attribute: the same scalar kind, and for nested relations /
/// user-defined values, equal type descriptors.
///
/// Zero-alloc: this is the per-tuple check behind
/// [`ArenaOpError::TypeMismatch`].
fn same_scalar_kind(a: ScalarRef<'_>, b: ScalarRef<'_>) -> bool {
    match (a, b) {
        (ScalarRef::Int(_), ScalarRef::Int(_))
        | (ScalarRef::Float(_), ScalarRef::Float(_))
        | (ScalarRef::Bool(_), ScalarRef::Bool(_))
        | (ScalarRef::Text(_), ScalarRef::Text(_))
        | (ScalarRef::Binary(_), ScalarRef::Binary(_)) => true,
        (ScalarRef::Relation(a), ScalarRef::Relation(b)) => a.heading() == b.heading(),
        (
            ScalarRef::UserDefined {
                type_def: a_type, ..
            },
            ScalarRef::UserDefined {
                type_def: b_type, ..
            },
        ) => a_type == b_type,
        _ => false,
    }
}

/// Builds the natural-join heading of `a` and `b`: `a`'s attributes plus
/// `b`'s non-common attributes, in [`TupleType`]'s canonical (name-sorted)
/// order. On a name collision the left relation's attribute (and its type)
/// wins — the same rule [`Relation::join`](crate::values::Relation::join)
/// applies.
pub(crate) fn natural_join_heading(a: &SlotRelation<'_>, b: &SlotRelation<'_>) -> TupleType {
    let mut heading = a.heading().clone();
    for (name, scalar_type) in b.heading().attributes() {
        if !heading.has_attribute(name) {
            heading = heading.with_attribute(name.clone(), scalar_type.clone());
        }
    }
    heading
}

/// Builds the projection heading for `attrs` against `heading`.
///
/// # Errors
///
/// Returns [`ArenaOpError::UnknownAttribute`] for the first attribute name
/// that does not resolve against the input heading.
pub(crate) fn project_heading(
    heading: &TupleType,
    attrs: &[&str],
) -> Result<TupleType, ArenaOpError> {
    let mut projected = TupleType::new();
    for attr in attrs {
        match heading.get_attribute_type(attr) {
            Some(scalar_type) => {
                projected = projected.with_attribute(*attr, scalar_type.clone());
            }
            None => return Err(ArenaOpError::UnknownAttribute((*attr).to_string())),
        }
    }
    Ok(projected)
}

/// Hashes the common-attribute projection of `view`: the natural-join key
/// hash. Key semantics match the owned [`Relation::join`](crate::values::Relation::join):
/// **all** commonly-named attributes form the key.
fn join_key_hash(view: TupleView<'_>, a_heading: &TupleType, b_heading: &TupleType) -> u64 {
    let mut hasher = new_hasher();
    for name in a_heading.attribute_names() {
        if b_heading.has_attribute(name) {
            view.get(name).hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Compares the natural-join keys (all commonly-named attributes) of two
/// tuples for equality.
fn join_keys_equal(
    build: TupleView<'_>,
    probe: TupleView<'_>,
    a_heading: &TupleType,
    b_heading: &TupleType,
) -> bool {
    a_heading
        .attribute_names()
        .filter(|name| b_heading.has_attribute(name))
        .all(|name| build.get(name) == probe.get(name))
}

// ---------------------------------------------------------------------------
// `*_into` cores: caller-sized outputs, zero heap allocation on the hot path
//
// Each core appends to a caller-provided `out: &mut SlotRelation`. The caller
// picks the output capacity from the documented upper bound; the core itself
// performs no heap allocation. The public `*_in` wrappers below size outputs
// at those bounds.
// ---------------------------------------------------------------------------

/// Restrict (σ) core: appends to `out` the tuples of `input` satisfying
/// `pred`, deduplicated (set semantics).
///
/// Upper bound for `out`'s capacity: `input.len()`.
pub(crate) fn restrict_into<'a>(
    input: &SlotRelation<'a>,
    pred: &dyn Fn(TupleView<'a>) -> bool,
    out: &mut SlotRelation<'a>,
    arena: &'a Arena,
) -> Result<(), ArenaOpError> {
    let mut seen = ArenaSet::new(arena, input.len())?;
    for view in input.iter() {
        if pred(view) && seen.insert(view) {
            append_tuple(out, view.values)?;
        }
    }
    Ok(())
}

/// Project (π) core: appends to `out` the projection of every input tuple
/// onto `out`'s heading, deduplicated.
///
/// Upper bound for `out`'s capacity: `input.len()`.
pub(crate) fn project_into<'a>(
    input: &SlotRelation<'a>,
    out: &mut SlotRelation<'a>,
    arena: &'a Arena,
) -> Result<(), ArenaOpError> {
    let out_heading = out.heading();
    let degree = out_heading.degree();
    let mut seen = ArenaSet::new(arena, input.len())?;
    let scratch = arena.alloc_slice_mut(degree, ScalarRef::Int(0))?;
    for view in input.iter() {
        for (i, name) in out_heading.attribute_names().enumerate() {
            scratch[i] = view
                .get(name)
                .ok_or_else(|| ArenaOpError::UnknownAttribute(name.clone()))?;
        }
        // Fresh arena slice per tuple: the candidate key must outlive the
        // scratch buffer, which is reused for the next tuple.
        let key = arena.alloc_slice(scratch)?;
        let candidate = TupleView {
            heading: out_heading,
            values: key,
        };
        if seen.insert(candidate) {
            append_tuple(out, key)?;
        }
    }
    Ok(())
}

/// Union (∪) core: appends `a`'s tuples then `b`'s tuples not already in `a`,
/// deduplicated.
///
/// Upper bound for `out`'s capacity: `a.len() + b.len()`.
///
/// # Errors
///
/// Returns [`ArenaOpError::HeadingMismatch`] when the inputs are not
/// union-compatible.
pub(crate) fn union_into<'a>(
    a: &SlotRelation<'a>,
    b: &SlotRelation<'a>,
    out: &mut SlotRelation<'a>,
    arena: &'a Arena,
) -> Result<(), ArenaOpError> {
    if !headings_identical(a.heading(), b.heading()) {
        return Err(ArenaOpError::HeadingMismatch);
    }
    let mut seen = ArenaSet::new(arena, a.len() + b.len())?;
    for view in a.iter().chain(b.iter()) {
        if seen.insert(view) {
            append_tuple(out, view.values)?;
        }
    }
    Ok(())
}

/// Difference (−) core: appends `a`'s tuples that are absent from `b`,
/// deduplicated.
///
/// Upper bound for `out`'s capacity: `a.len()`.
///
/// # Errors
///
/// Returns [`ArenaOpError::HeadingMismatch`] when the inputs are not
/// union-compatible.
pub(crate) fn difference_into<'a>(
    a: &SlotRelation<'a>,
    b: &SlotRelation<'a>,
    out: &mut SlotRelation<'a>,
    arena: &'a Arena,
) -> Result<(), ArenaOpError> {
    if !headings_identical(a.heading(), b.heading()) {
        return Err(ArenaOpError::HeadingMismatch);
    }
    let mut excluded = ArenaSet::new(arena, b.len())?;
    for view in b.iter() {
        excluded.insert(view);
    }
    let mut seen = ArenaSet::new(arena, a.len())?;
    for view in a.iter() {
        if !excluded.contains(view) && seen.insert(view) {
            append_tuple(out, view.values)?;
        }
    }
    Ok(())
}

/// Intersect (∩) core: appends `a`'s tuples that are present in `b`,
/// deduplicated.
///
/// Upper bound for `out`'s capacity: `min(a.len(), b.len())`.
///
/// # Errors
///
/// Returns [`ArenaOpError::HeadingMismatch`] when the inputs are not
/// union-compatible.
pub(crate) fn intersect_into<'a>(
    a: &SlotRelation<'a>,
    b: &SlotRelation<'a>,
    out: &mut SlotRelation<'a>,
    arena: &'a Arena,
) -> Result<(), ArenaOpError> {
    if !headings_identical(a.heading(), b.heading()) {
        return Err(ArenaOpError::HeadingMismatch);
    }
    let mut in_b = ArenaSet::new(arena, b.len())?;
    for view in b.iter() {
        in_b.insert(view);
    }
    let mut seen = ArenaSet::new(arena, a.len().min(b.len()))?;
    for view in a.iter() {
        if in_b.contains(view) && seen.insert(view) {
            append_tuple(out, view.values)?;
        }
    }
    Ok(())
}

/// Natural join core: appends to `out` one combined tuple per matching
/// a/b pair, deduplicated.
///
/// The hash table is built over the **smaller** input; with no common
/// attributes every probe matches every build tuple (Cartesian product).
/// Tuple assembly is name-based — each attribute resolves from whichever
/// side carries it — so left/right placement is correct regardless of which
/// input was the hash build side. The combined heading keeps the left
/// relation's attribute (and its type) on a name collision, matching the
/// owned [`Relation::join`](crate::values::Relation::join).
///
/// Upper bound for `out`'s capacity: `a.len() * b.len()` (the Cartesian
/// bound). A caller that knows a tighter bound may size `out` smaller and
/// will get [`ArenaOpError::CapacityError`] if the result overflows it.
pub(crate) fn join_into<'a>(
    a: &SlotRelation<'a>,
    b: &SlotRelation<'a>,
    out: &mut SlotRelation<'a>,
    arena: &'a Arena,
) -> Result<(), ArenaOpError> {
    let a_heading = a.heading();
    let b_heading = b.heading();
    let out_heading = out.heading();
    let out_degree = out_heading.degree();

    let (build, probe) = if a.len() <= b.len() { (a, b) } else { (b, a) };

    let mut map = ArenaMap::new(arena, build.len())?;
    for view in build.iter() {
        map.insert(join_key_hash(view, a_heading, b_heading), view);
    }

    // The natural join of two sets is a set, but `SlotRelation` does not
    // enforce it: eliminate duplicates explicitly (matches `Relation::join`).
    let mut emitted = ArenaSet::new(arena, out.capacity())?;
    let scratch = arena.alloc_slice_mut(out_degree, ScalarRef::Int(0))?;

    for probe_view in probe.iter() {
        let hash = join_key_hash(probe_view, a_heading, b_heading);
        map.for_each_match(
            hash,
            probe_view,
            |build_view, other| join_keys_equal(build_view, other, a_heading, b_heading),
            |build_view| {
                // Combined tuple, positional in the output heading's canonical
                // order. Common attributes are equal on both sides by the key
                // match, so either side's value is the a-side value.
                for (i, name) in out_heading.attribute_names().enumerate() {
                    scratch[i] = build_view
                        .get(name)
                        .or_else(|| probe_view.get(name))
                        .ok_or_else(|| ArenaOpError::UnknownAttribute(name.clone()))?;
                }
                // Fresh arena slice per emitted tuple: the key must outlive
                // the scratch buffer, which is reused for the next tuple.
                let key = arena.alloc_slice(scratch)?;
                let candidate = TupleView {
                    heading: out_heading,
                    values: key,
                };
                if emitted.insert(candidate) {
                    append_tuple(out, key)?;
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

/// Extend core: appends to `out` every input tuple plus the computed
/// attribute `name`.
///
/// The new attribute's type is inferred from the first tuple's computed
/// value; every subsequent computed value must have a compatible type.
///
/// Upper bound for `out`'s capacity: `input.len()`.
///
/// # Errors
///
/// - [`ArenaOpError::AttributeExists`] when `name` is already an attribute of
///   the input.
/// - [`ArenaOpError::CannotInferType`] when the input is empty (there is no
///   tuple to infer the new attribute's type from).
/// - [`ArenaOpError::TypeMismatch`] when a computed value's type differs
///   from the inferred attribute type.
/// - [`ArenaOpError::ArityMismatch`] when the output heading's degree is not
///   the input degree plus one.
pub(crate) fn extend_into<'a>(
    input: &SlotRelation<'a>,
    name: &str,
    f: &dyn Fn(TupleView<'a>) -> ScalarRef<'a>,
    out: &mut SlotRelation<'a>,
    arena: &'a Arena,
) -> Result<(), ArenaOpError> {
    if input.heading().has_attribute(name) {
        return Err(ArenaOpError::AttributeExists(name.to_string()));
    }
    let first = input
        .iter()
        .next()
        .ok_or_else(|| ArenaOpError::CannotInferType(name.to_string()))?;
    // The new attribute's type is inferred from the first tuple's computed
    // value (the signature carries no `ScalarType`); every subsequent
    // computed value must have a compatible type.
    let first_computed = f(first);
    let inferred = scalar_type_of(first_computed);
    let out_heading = out.heading();
    let input_degree = input.heading().degree();
    if out_heading.degree() != input_degree + 1 {
        return Err(ArenaOpError::ArityMismatch {
            expected: input_degree + 1,
            found: out_heading.degree(),
        });
    }
    let degree = out_heading.degree();
    let scratch = arena.alloc_slice_mut(degree, ScalarRef::Int(0))?;
    for view in input.iter() {
        let computed = f(view);
        if !same_scalar_kind(first_computed, computed) {
            return Err(ArenaOpError::TypeMismatch {
                attribute: name.to_string(),
                expected: inferred.name().to_string(),
                found: scalar_type_of(computed).name().to_string(),
            });
        }
        for (i, attr) in out_heading.attribute_names().enumerate() {
            scratch[i] = if attr == name {
                computed
            } else {
                view.get(attr)
                    .ok_or_else(|| ArenaOpError::UnknownAttribute(attr.clone()))?
            };
        }
        // Fresh arena slice per tuple: the candidate key must outlive the
        // scratch buffer, which is reused for the next tuple.
        let key = arena.alloc_slice(scratch)?;
        append_tuple(out, key)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public operators: arena-native counterparts of the owned algebra.
// ---------------------------------------------------------------------------

/// Restrict (σ): keeps the tuples of `input` for which `pred` returns `true`.
///
/// TTM: σ from Codd's algebra. Set semantics — duplicates are eliminated,
/// matching the owned [`Relation::restrict`](crate::algebra::Relation::restrict).
///
/// The output reserves `input.len()` slots (upper bound) and reuses the input
/// heading, so the per-call setup performs no allocation beyond the arena.
///
/// # Errors
///
/// - [`ArenaOpError::AllocError`] if the arena cannot back the output.
/// - [`ArenaOpError::CapacityError`] if the output overflows (cannot happen
///   at the documented bound unless `pred` misbehaves — it cannot create
///   tuples).
pub fn restrict_in<'a>(
    input: &SlotRelation<'a>,
    pred: &dyn Fn(TupleView<'a>) -> bool,
    arena: &'a Arena,
) -> Result<SlotRelation<'a>, ArenaOpError> {
    let mut out = SlotRelation::with_capacity(arena, input.heading(), input.len());
    restrict_into(input, pred, &mut out, arena)?;
    Ok(out)
}

/// Project (π): keeps the named attributes of every tuple of `input`.
///
/// TTM: π from Codd's algebra. Set semantics — duplicates are eliminated,
/// matching the owned [`Relation::project`](crate::algebra::Relation::project).
/// Attribute *names* resolve against the input heading; values are stored
/// positionally in the result heading's canonical order.
///
/// The output reserves `input.len()` slots (upper bound). Building the
/// projected heading allocates `O(attrs.len())` outside the arena (owned
/// [`TupleType`] metadata); the finished heading is then placed in the arena.
///
/// # Errors
///
/// - [`ArenaOpError::UnknownAttribute`] naming the first attribute in
///   `attrs` that does not resolve against the input heading.
/// - [`ArenaOpError::AllocError`] if the arena cannot back the output.
/// - [`ArenaOpError::CapacityError`] if the output overflows (cannot happen
///   at the documented bound).
pub fn project_in<'a>(
    input: &SlotRelation<'a>,
    attrs: &[&str],
    arena: &'a Arena,
) -> Result<SlotRelation<'a>, ArenaOpError> {
    let heading = project_heading(input.heading(), attrs)?;
    let heading: &'a TupleType = arena.alloc_value(heading)?;
    let mut out = SlotRelation::with_capacity(arena, heading, input.len());
    project_into(input, &mut out, arena)?;
    Ok(out)
}

/// Union (∪): tuples of `a` plus tuples of `b`, deduplicated.
///
/// TTM: set union. Both inputs must share one identical heading (attribute
/// names *and* types) — union compatibility.
///
/// The output reserves `a.len() + b.len()` slots (upper bound) and reuses
/// `a`'s heading, so the per-call setup performs no allocation beyond the
/// arena.
///
/// # Errors
///
/// - [`ArenaOpError::HeadingMismatch`] when the inputs are not
///   union-compatible.
/// - [`ArenaOpError::AllocError`] if the arena cannot back the output, or if
///   the capacity bound overflows `usize`.
/// - [`ArenaOpError::CapacityError`] if the output overflows (cannot happen
///   at the documented bound).
pub fn union_in<'a>(
    a: &SlotRelation<'a>,
    b: &SlotRelation<'a>,
    arena: &'a Arena,
) -> Result<SlotRelation<'a>, ArenaOpError> {
    let capacity = a
        .len()
        .checked_add(b.len())
        .ok_or(AllocError::SizeOverflow(a.len()))?;
    let mut out = SlotRelation::with_capacity(arena, a.heading(), capacity);
    union_into(a, b, &mut out, arena)?;
    Ok(out)
}

/// Difference (−): tuples of `a` that are absent from `b`, deduplicated.
///
/// TTM: set difference. Both inputs must share one identical heading
/// (union compatibility).
///
/// The output reserves `a.len()` slots (upper bound) and reuses `a`'s
/// heading, so the per-call setup performs no allocation beyond the arena.
///
/// # Errors
///
/// - [`ArenaOpError::HeadingMismatch`] when the inputs are not
///   union-compatible.
/// - [`ArenaOpError::AllocError`] if the arena cannot back the output.
/// - [`ArenaOpError::CapacityError`] if the output overflows (cannot happen
///   at the documented bound).
pub fn difference_in<'a>(
    a: &SlotRelation<'a>,
    b: &SlotRelation<'a>,
    arena: &'a Arena,
) -> Result<SlotRelation<'a>, ArenaOpError> {
    let mut out = SlotRelation::with_capacity(arena, a.heading(), a.len());
    difference_into(a, b, &mut out, arena)?;
    Ok(out)
}

/// Intersect (∩): tuples of `a` that are present in `b`, deduplicated.
///
/// TTM: set intersection. Both inputs must share one identical heading
/// (union compatibility).
///
/// The output reserves `a.len()` slots (a sound upper bound — the true
/// bound is `min(a.len(), b.len())`) and reuses `a`'s heading, so the
/// per-call setup performs no allocation beyond the arena.
///
/// # Errors
///
/// - [`ArenaOpError::HeadingMismatch`] when the inputs are not
///   union-compatible.
/// - [`ArenaOpError::AllocError`] if the arena cannot back the output.
/// - [`ArenaOpError::CapacityError`] if the output overflows (cannot happen
///   at the documented bound).
pub fn intersect_in<'a>(
    a: &SlotRelation<'a>,
    b: &SlotRelation<'a>,
    arena: &'a Arena,
) -> Result<SlotRelation<'a>, ArenaOpError> {
    let mut out = SlotRelation::with_capacity(arena, a.heading(), a.len());
    intersect_into(a, b, &mut out, arena)?;
    Ok(out)
}

/// Natural join (⨝): one combined tuple per matching a/b pair.
///
/// TTM: the natural join. The join key is **all** commonly-named attributes —
/// the same key semantics as the owned
/// [`Relation::join`](crate::values::Relation::join). The combined heading is
/// `a`'s attributes plus `b`'s non-common attributes; on a name collision the
/// left relation's attribute (and its type) wins. With no common attributes
/// this is the Cartesian product.
///
/// Set semantics: duplicate output tuples are eliminated, matching the owned
/// operator. The hash table is built over the **smaller** input.
///
/// The output reserves `a.len() * b.len()` slots (the Cartesian upper
/// bound). Building the combined heading allocates `O(degree)` outside the
/// arena (owned [`TupleType`] metadata); the finished heading is then placed
/// in the arena.
///
/// # Errors
///
/// - [`ArenaOpError::AllocError`] if the arena cannot back the output, or if
///   the capacity bound overflows `usize`.
/// - [`ArenaOpError::CapacityError`] if the output overflows (cannot happen
///   at the documented bound).
pub fn join_in<'a>(
    a: &SlotRelation<'a>,
    b: &SlotRelation<'a>,
    arena: &'a Arena,
) -> Result<SlotRelation<'a>, ArenaOpError> {
    let capacity = a
        .len()
        .checked_mul(b.len())
        .ok_or(AllocError::SizeOverflow(a.len()))?;
    let heading = natural_join_heading(a, b);
    let heading: &'a TupleType = arena.alloc_value(heading)?;
    let mut out = SlotRelation::with_capacity(arena, heading, capacity);
    join_into(a, b, &mut out, arena)?;
    Ok(out)
}

/// Extend: appends one computed attribute `name` to every tuple of `input`.
///
/// TTM: the EXTEND operator. `f` computes the new attribute's value from each
/// tuple. Unlike the owned
/// [`Relation::extend`](crate::algebra::Relation::extend), the signature
/// carries no [`ScalarType`]: the new attribute's type is **inferred from the
/// first tuple's computed value**, and every subsequent computed value must
/// have a compatible type.
///
/// The output reserves `input.len()` slots (upper bound). Building the
/// extended heading allocates `O(degree)` outside the arena (owned
/// [`TupleType`] metadata); the finished heading is then placed in the arena.
///
/// # Errors
///
/// - [`ArenaOpError::AttributeExists`] when `name` is already an attribute of
///   the input.
/// - [`ArenaOpError::CannotInferType`] when the input is empty — there is no
///   tuple to infer the new attribute's type from.
/// - [`ArenaOpError::TypeMismatch`] when a computed value's type differs from
///   the inferred attribute type.
/// - [`ArenaOpError::AllocError`] if the arena cannot back the output.
/// - [`ArenaOpError::CapacityError`] if the output overflows (cannot happen
///   at the documented bound).
pub fn extend_in<'a>(
    input: &SlotRelation<'a>,
    name: &str,
    f: &dyn Fn(TupleView<'a>) -> ScalarRef<'a>,
    arena: &'a Arena,
) -> Result<SlotRelation<'a>, ArenaOpError> {
    if input.heading().has_attribute(name) {
        return Err(ArenaOpError::AttributeExists(name.to_string()));
    }
    let first = input
        .iter()
        .next()
        .ok_or_else(|| ArenaOpError::CannotInferType(name.to_string()))?;
    let inferred = scalar_type_of(f(first));
    let heading = input.heading().clone().with_attribute(name, inferred);
    let heading: &'a TupleType = arena.alloc_value(heading)?;
    let mut out = SlotRelation::with_capacity(arena, heading, input.len());
    extend_into(input, name, f, &mut out, arena)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    // ------------------------------------------------------------------
    // Counting allocator: the zero-alloc proof.
    //
    // Counts `alloc` calls on the *current thread only*, so other libtest
    // threads allocating concurrently can never pollute the measurement.
    // Delegates every call to the system allocator, so behavior is otherwise
    // identical to the default allocator. (zahttp-style proof.)
    // ------------------------------------------------------------------
    mod counting {
        extern crate std;

        use core::alloc::{GlobalAlloc, Layout};
        use core::cell::Cell;

        std::thread_local! {
            static ALLOC_COUNT: Cell<usize> = const { Cell::new(0) };
        }

        struct CountingAllocator;

        unsafe impl GlobalAlloc for CountingAllocator {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                ALLOC_COUNT.with(|count| count.set(count.get().wrapping_add(1)));
                // SAFETY: straight delegation to the system allocator.
                unsafe { std::alloc::System.alloc(layout) }
            }

            unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                // SAFETY: `ptr`/`layout` came from `alloc` above.
                unsafe { std::alloc::System.dealloc(ptr, layout) }
            }
        }

        #[global_allocator]
        static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

        /// Resets the current thread's allocation count to zero.
        ///
        /// Touches the thread-local first so its maiden access can never be
        /// measured.
        pub fn reset() {
            let _ = count();
            ALLOC_COUNT.with(|count| count.set(0));
        }

        /// Allocation count on the current thread since the last [`reset`].
        pub fn count() -> usize {
            ALLOC_COUNT.with(|count| count.get())
        }
    }

    // ------------------------------------------------------------------
    // Test data
    // ------------------------------------------------------------------

    /// 12 employees, `dept_id` cycling through 10/20/30 (4 each).
    fn employees_relation() -> Relation {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);
        let mut relation = Relation::new(RelationType::new(heading));
        let names = [
            "Alice", "Bob", "Carol", "Dave", "Erin", "Frank", "Grace", "Heidi", "Ivan", "Judy",
            "Karl", "Lena",
        ];
        let dept_ids = [10i64, 20, 30];
        for (i, name) in names.iter().enumerate() {
            relation
                .insert(tuple! { emp_id: i as i64 + 1, name: *name, dept_id: dept_ids[i % 3] })
                .unwrap();
        }
        relation
    }

    /// 4 departments; dept 40 ("Empty") has no employees.
    fn departments_relation() -> Relation {
        let heading = TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("dept_name", ScalarType::String);
        let mut relation = Relation::new(RelationType::new(heading));
        for (id, name) in [
            (10i64, "Engineering"),
            (20, "Sales"),
            (30, "Marketing"),
            (40, "Empty"),
        ] {
            relation
                .insert(tuple! { dept_id: id, dept_name: name })
                .unwrap();
        }
        relation
    }

    /// Single-attribute integer relation over `values`.
    fn small_int_relation(values: &[i64]) -> Relation {
        let heading = TupleType::new().with_attribute("n", ScalarType::Int);
        let mut relation = Relation::new(RelationType::new(heading));
        for value in values {
            relation.insert(tuple! { n: *value }).unwrap();
        }
        relation
    }

    /// Borrows an owned relation into arena slots.
    fn slots_of<'a>(relation: &'a Relation, arena: &'a Arena, capacity: usize) -> SlotRelation<'a> {
        SlotRelation::from_relation(relation, arena, capacity).unwrap()
    }

    /// Collects the `n` values of every tuple in `relation` (order-free).
    fn int_column(relation: &SlotRelation, attr: &str) -> Vec<i64> {
        let mut values: Vec<i64> = relation
            .iter()
            .map(|view| match view.get(attr) {
                Some(ScalarRef::Int(n)) => n,
                other => panic!("expected Int, found {other:?}"),
            })
            .collect();
        values.sort_unstable();
        values
    }

    // ------------------------------------------------------------------
    // restrict_in
    // ------------------------------------------------------------------

    #[test]
    fn restrict_keeps_matching_tuples() {
        let arena = Arena::new();
        let employees = employees_relation();
        let input = slots_of(&employees, &arena, 32);

        let result = restrict_in(
            &input,
            &|view: TupleView| view.get("dept_id") == Some(ScalarRef::Int(10)),
            &arena,
        )
        .unwrap();

        assert_eq!(result.len(), 4);
        assert!(core::ptr::eq(result.heading(), input.heading()));
        for view in result.iter() {
            assert_eq!(view.get("dept_id"), Some(ScalarRef::Int(10)));
        }
    }

    #[test]
    fn restrict_eliminates_physical_duplicates() {
        let arena = Arena::new();
        let employees = employees_relation();
        let mut input = slots_of(&employees, &arena, 64);
        // Duplicate the first tuple physically: `SlotRelation` never dedupes.
        let first_values: Vec<ScalarRef> = input.iter().next().unwrap().values.to_vec();
        input.insert(&first_values).unwrap();
        assert_eq!(input.len(), 13);

        let result = restrict_in(&input, &|_| true, &arena).unwrap();

        assert_eq!(result.len(), 12);
    }

    #[test]
    fn restrict_empty_result() {
        let arena = Arena::new();
        let employees = employees_relation();
        let input = slots_of(&employees, &arena, 32);

        let result = restrict_in(&input, &|_| false, &arena).unwrap();

        assert_eq!(result.len(), 0);
    }

    // ------------------------------------------------------------------
    // project_in
    // ------------------------------------------------------------------

    #[test]
    fn project_selects_attributes_and_dedups() {
        let arena = Arena::new();
        let employees = employees_relation();
        let input = slots_of(&employees, &arena, 32);

        let result = project_in(&input, &["dept_id"], &arena).unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result.heading().degree(), 1);
        assert!(result.heading().has_attribute("dept_id"));
        assert_eq!(int_column(&result, "dept_id"), [10, 20, 30]);
    }

    #[test]
    fn project_unknown_attribute_errors_without_panicking() {
        let arena = Arena::new();
        let employees = employees_relation();
        let input = slots_of(&employees, &arena, 32);

        let err = project_in(&input, &["dept_id", "nope"], &arena).unwrap_err();

        match err {
            ArenaOpError::UnknownAttribute(name) => assert_eq!(name, "nope"),
            other => panic!("expected UnknownAttribute, got {other:?}"),
        }
    }

    #[test]
    fn project_onto_no_attributes_yields_single_empty_tuple() {
        let arena = Arena::new();
        let employees = employees_relation();
        let input = slots_of(&employees, &arena, 32);

        // π over no attributes: every tuple projects to the 0-tuple, and set
        // semantics collapse them to one (DEE, not DUM — the input is
        // nonempty).
        let result = project_in(&input, &[], &arena).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result.heading().degree(), 0);
    }

    // ------------------------------------------------------------------
    // union_in / difference_in / intersect_in
    // ------------------------------------------------------------------

    #[test]
    fn union_combines_and_dedups() {
        let arena = Arena::new();
        let a = small_int_relation(&[1, 2, 3]);
        let b = small_int_relation(&[3, 4, 5]);
        let a_slots = slots_of(&a, &arena, 8);
        let b_slots = slots_of(&b, &arena, 8);

        let result = union_in(&a_slots, &b_slots, &arena).unwrap();

        assert_eq!(result.len(), 5);
        assert_eq!(int_column(&result, "n"), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn union_rejects_mismatched_headings() {
        let arena = Arena::new();
        let a = small_int_relation(&[1]);
        let heading = TupleType::new().with_attribute("n", ScalarType::String);
        let mut b = Relation::new(RelationType::new(heading));
        b.insert(tuple! { n: "x" }).unwrap();
        let a_slots = slots_of(&a, &arena, 8);
        let b_slots = slots_of(&b, &arena, 8);

        let err = union_in(&a_slots, &b_slots, &arena).unwrap_err();

        assert!(matches!(err, ArenaOpError::HeadingMismatch));
    }

    #[test]
    fn difference_keeps_a_minus_b() {
        let arena = Arena::new();
        let a = small_int_relation(&[1, 2, 3]);
        let b = small_int_relation(&[3, 4]);
        let a_slots = slots_of(&a, &arena, 8);
        let b_slots = slots_of(&b, &arena, 8);

        let result = difference_in(&a_slots, &b_slots, &arena).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(int_column(&result, "n"), [1, 2]);
    }

    #[test]
    fn difference_rejects_mismatched_headings() {
        let arena = Arena::new();
        let a = small_int_relation(&[1]);
        let heading = TupleType::new()
            .with_attribute("n", ScalarType::Int)
            .with_attribute("extra", ScalarType::Int);
        let mut b = Relation::new(RelationType::new(heading));
        b.insert(tuple! { n: 1i64, extra: 2i64 }).unwrap();
        let a_slots = slots_of(&a, &arena, 8);
        let b_slots = slots_of(&b, &arena, 8);

        let err = difference_in(&a_slots, &b_slots, &arena).unwrap_err();

        assert!(matches!(err, ArenaOpError::HeadingMismatch));
    }

    #[test]
    fn intersect_keeps_common_tuples() {
        let arena = Arena::new();
        let a = small_int_relation(&[1, 2, 3]);
        let b = small_int_relation(&[2, 3, 4]);
        let a_slots = slots_of(&a, &arena, 8);
        let b_slots = slots_of(&b, &arena, 8);

        let result = intersect_in(&a_slots, &b_slots, &arena).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(int_column(&result, "n"), [2, 3]);
    }

    #[test]
    fn intersect_rejects_mismatched_headings() {
        let arena = Arena::new();
        let a = small_int_relation(&[1]);
        let heading = TupleType::new().with_attribute("m", ScalarType::Int);
        let mut b = Relation::new(RelationType::new(heading));
        b.insert(tuple! { m: 1i64 }).unwrap();
        let a_slots = slots_of(&a, &arena, 8);
        let b_slots = slots_of(&b, &arena, 8);

        let err = intersect_in(&a_slots, &b_slots, &arena).unwrap_err();

        assert!(matches!(err, ArenaOpError::HeadingMismatch));
    }

    #[test]
    fn set_operators_eliminate_physical_duplicates() {
        let arena = Arena::new();
        let a = small_int_relation(&[1, 2]);
        let b = small_int_relation(&[2, 3]);
        let mut a_slots = slots_of(&a, &arena, 16);
        let first_values: Vec<ScalarRef> = a_slots.iter().next().unwrap().values.to_vec();
        a_slots.insert(&first_values).unwrap();
        let b_slots = slots_of(&b, &arena, 8);

        let union = union_in(&a_slots, &b_slots, &arena).unwrap();
        let difference = difference_in(&a_slots, &b_slots, &arena).unwrap();
        let intersect = intersect_in(&a_slots, &b_slots, &arena).unwrap();

        assert_eq!(int_column(&union, "n"), [1, 2, 3]);
        assert_eq!(int_column(&difference, "n"), [1]);
        assert_eq!(int_column(&intersect, "n"), [2]);
    }

    // ------------------------------------------------------------------
    // join_in
    // ------------------------------------------------------------------

    #[test]
    fn join_matches_on_all_common_attributes() {
        let arena = Arena::new();
        let employees = employees_relation();
        let departments = departments_relation();
        let emp = slots_of(&employees, &arena, 32);
        let dept = slots_of(&departments, &arena, 8);

        let result = join_in(&emp, &dept, &arena).unwrap();

        // Dept 40 ("Empty") has no employees.
        assert_eq!(result.len(), 12);
        let heading = result.heading();
        assert_eq!(heading.degree(), 4);
        for attr in ["emp_id", "name", "dept_id", "dept_name"] {
            assert!(heading.has_attribute(attr), "missing {attr}");
        }
        for view in result.iter() {
            let expected_name = match view.get("dept_id") {
                Some(ScalarRef::Int(10)) => Some(ScalarRef::Text("Engineering")),
                Some(ScalarRef::Int(20)) => Some(ScalarRef::Text("Sales")),
                Some(ScalarRef::Int(30)) => Some(ScalarRef::Text("Marketing")),
                other => panic!("unexpected dept_id {other:?}"),
            };
            assert_eq!(view.get("dept_name"), expected_name);
        }
    }

    #[test]
    fn join_without_common_attributes_is_cartesian() {
        let arena = Arena::new();
        let a = small_int_relation(&[1, 2]);
        let heading = TupleType::new().with_attribute("s", ScalarType::String);
        let mut b = Relation::new(RelationType::new(heading));
        b.insert(tuple! { s: "x" }).unwrap();
        b.insert(tuple! { s: "y" }).unwrap();
        let a_slots = slots_of(&a, &arena, 8);
        let b_slots = slots_of(&b, &arena, 8);

        let result = join_in(&a_slots, &b_slots, &arena).unwrap();

        assert_eq!(result.len(), 4);
        assert_eq!(result.heading().degree(), 2);
    }

    #[test]
    fn join_preserves_sides_when_right_is_smaller() {
        let arena = Arena::new();
        // `b` is smaller, so the hash table is built over the RIGHT input;
        // left/right placement in the output must still be correct.
        let a = small_int_relation(&[1, 2, 3, 4]);
        let heading = TupleType::new()
            .with_attribute("n", ScalarType::Int)
            .with_attribute("m", ScalarType::Int);
        let mut b = Relation::new(RelationType::new(heading));
        b.insert(tuple! { n: 2i64, m: 20i64 }).unwrap();
        b.insert(tuple! { n: 4i64, m: 40i64 }).unwrap();
        let a_slots = slots_of(&a, &arena, 8);
        let b_slots = slots_of(&b, &arena, 8);
        assert!(b_slots.len() < a_slots.len());

        let result = join_in(&a_slots, &b_slots, &arena).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(int_column(&result, "n"), [2, 4]);
        assert_eq!(int_column(&result, "m"), [20, 40]);
        assert_eq!(result.heading().degree(), 2);
    }

    #[test]
    fn join_left_wins_name_collisions() {
        let arena = Arena::new();
        // Same attribute name, different types: the key can never match
        // (Int != String), but the combined heading must keep the LEFT type.
        let a_heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("x", ScalarType::Int);
        let mut a = Relation::new(RelationType::new(a_heading));
        a.insert(tuple! { id: 1i64, x: 1i64 }).unwrap();
        let b_heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("x", ScalarType::String);
        let mut b = Relation::new(RelationType::new(b_heading));
        b.insert(tuple! { id: 1i64, x: "one" }).unwrap();
        let a_slots = slots_of(&a, &arena, 8);
        let b_slots = slots_of(&b, &arena, 8);

        let result = join_in(&a_slots, &b_slots, &arena).unwrap();

        assert_eq!(result.len(), 0);
        assert_eq!(
            result.heading().get_attribute_type("x"),
            Some(&ScalarType::Int)
        );
    }

    #[test]
    fn join_eliminates_duplicate_output_tuples() {
        let arena = Arena::new();
        // Duplicate join keys on both sides: 2 × 2 matches collapse to one
        // output tuple by set semantics.
        let a = small_int_relation(&[1, 1]);
        let b = small_int_relation(&[1, 1]);
        let mut a_slots = slots_of(&a, &arena, 8);
        let first_values: Vec<ScalarRef> = a_slots.iter().next().unwrap().values.to_vec();
        a_slots.insert(&first_values).unwrap();
        let b_slots = slots_of(&b, &arena, 8);

        let result = join_in(&a_slots, &b_slots, &arena).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn join_into_propagates_capacity_error() {
        let arena = Arena::new();
        let a = small_int_relation(&[1, 2]);
        let b = small_int_relation(&[1, 2]);
        let a_slots = slots_of(&a, &arena, 8);
        let b_slots = slots_of(&b, &arena, 8);

        // Caller-sized output below the Cartesian bound: the join must fail
        // with `CapacityError::Full`, not panic or truncate.
        let heading = natural_join_heading(&a_slots, &b_slots);
        let heading: &TupleType = arena.alloc_value(heading).unwrap();
        let mut out = SlotRelation::with_capacity(&arena, heading, 1);

        let err = join_into(&a_slots, &b_slots, &mut out, &arena).unwrap_err();

        match err {
            ArenaOpError::CapacityError(CapacityError::Full { capacity }) => {
                assert_eq!(capacity, 1);
            }
            other => panic!("expected CapacityError::Full, got {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // extend_in
    // ------------------------------------------------------------------

    #[test]
    fn extend_appends_computed_attribute() {
        let arena = Arena::new();
        let employees = employees_relation();
        let input = slots_of(&employees, &arena, 32);

        let result = extend_in(
            &input,
            "senior",
            &|view: TupleView| match view.get("emp_id") {
                Some(ScalarRef::Int(id)) => ScalarRef::Bool(id > 6),
                _ => ScalarRef::Bool(false),
            },
            &arena,
        )
        .unwrap();

        assert_eq!(result.len(), 12);
        assert_eq!(result.heading().degree(), 4);
        assert_eq!(
            result.heading().get_attribute_type("senior"),
            Some(&ScalarType::Bool)
        );
        let seniors = result
            .iter()
            .filter(|view| view.get("senior") == Some(ScalarRef::Bool(true)))
            .count();
        assert_eq!(seniors, 6);
    }

    #[test]
    fn extend_rejects_existing_attribute() {
        let arena = Arena::new();
        let employees = employees_relation();
        let input = slots_of(&employees, &arena, 32);

        let err = extend_in(&input, "name", &|_| ScalarRef::Int(0), &arena).unwrap_err();

        match err {
            ArenaOpError::AttributeExists(name) => assert_eq!(name, "name"),
            other => panic!("expected AttributeExists, got {other:?}"),
        }
    }

    #[test]
    fn extend_empty_input_cannot_infer_type() {
        let arena = Arena::new();
        let employees = employees_relation();
        let full = slots_of(&employees, &arena, 32);
        let empty = restrict_in(&full, &|_| false, &arena).unwrap();
        assert_eq!(empty.len(), 0);

        let err = extend_in(&empty, "senior", &|_| ScalarRef::Bool(true), &arena).unwrap_err();

        match err {
            ArenaOpError::CannotInferType(name) => assert_eq!(name, "senior"),
            other => panic!("expected CannotInferType, got {other:?}"),
        }
    }

    #[test]
    fn extend_rejects_inconsistent_computed_types() {
        let arena = Arena::new();
        let employees = employees_relation();
        let input = slots_of(&employees, &arena, 32);

        // Exactly one tuple computes an Int, the rest compute Bools. The
        // inferred type depends on hash iteration order, but a `TypeMismatch`
        // fires either way.
        let err = extend_in(
            &input,
            "mixed",
            &|view: TupleView| match view.get("emp_id") {
                Some(ScalarRef::Int(1)) => ScalarRef::Int(0),
                _ => ScalarRef::Bool(false),
            },
            &arena,
        )
        .unwrap_err();

        match err {
            ArenaOpError::TypeMismatch {
                attribute,
                expected,
                found,
            } => {
                assert_eq!(attribute, "mixed");
                assert_ne!(expected, found);
                assert!(expected == "Int" || expected == "Bool");
                assert!(found == "Int" || found == "Bool");
            }
            other => panic!("expected TypeMismatch, got {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // Differential test: owned pipeline vs arena pipeline
    // ------------------------------------------------------------------

    #[test]
    fn pipeline_matches_owned_operators() {
        let employees = employees_relation();
        let departments = departments_relation();

        // Owned pipeline.
        let owned = employees
            .join(&departments)
            .unwrap()
            .restrict(|tuple| tuple.get_typed::<i64>("dept_id").unwrap() == 10)
            .project(&["dept_name", "name"]);

        // Arena pipeline.
        let arena = Arena::new();
        let emp_slots = slots_of(&employees, &arena, 32);
        let dept_slots = slots_of(&departments, &arena, 8);
        let joined = join_in(&emp_slots, &dept_slots, &arena).unwrap();
        let restricted = restrict_in(
            &joined,
            &|view: TupleView| view.get("dept_id") == Some(ScalarRef::Int(10)),
            &arena,
        )
        .unwrap();
        let projected = project_in(&restricted, &["dept_name", "name"], &arena).unwrap();
        let back = projected.to_relation().unwrap();

        assert_eq!(back, owned);
    }

    // ------------------------------------------------------------------
    // Zero-alloc proof: join -> restrict -> project performs no heap
    // allocation outside the arena after pre-sizing.
    // ------------------------------------------------------------------

    #[test]
    fn pipeline_performs_zero_heap_allocations() {
        // -- Setup (allocations allowed): relations, slots, arena, and all
        //    caller-sized outputs are prepared BEFORE the measurement window.
        let arena = Arena::with_chunk_size(1 << 20);
        let employees = employees_relation();
        let departments = departments_relation();
        let emp_slots = slots_of(&employees, &arena, 32);
        let dept_slots = slots_of(&departments, &arena, 8);

        let join_heading: &TupleType = arena
            .alloc_value(natural_join_heading(&emp_slots, &dept_slots))
            .unwrap();
        let join_capacity = emp_slots.len() * dept_slots.len();
        let mut join_out = SlotRelation::with_capacity(&arena, join_heading, join_capacity);
        let mut restrict_out = SlotRelation::with_capacity(&arena, join_heading, join_capacity);
        let project_heading: &TupleType = arena
            .alloc_value(project_heading(join_heading, &["dept_name", "name"]).unwrap())
            .unwrap();
        let mut project_out = SlotRelation::with_capacity(&arena, project_heading, join_capacity);

        // -- Measurement: the per-tuple hot path. --
        counting::reset();
        join_into(&emp_slots, &dept_slots, &mut join_out, &arena).unwrap();
        restrict_into(
            &join_out,
            &|view: TupleView| view.get("dept_id") == Some(ScalarRef::Int(10)),
            &mut restrict_out,
            &arena,
        )
        .unwrap();
        project_into(&restrict_out, &mut project_out, &arena).unwrap();
        let allocations = counting::count();

        assert_eq!(
            allocations, 0,
            "arena pipeline performed {allocations} heap allocations outside the arena"
        );

        // Sanity: the pipeline still produced the right relation
        // (the 4 employees of dept 10, projected onto dept_name + name).
        assert_eq!(project_out.len(), 4);
        let mut names: Vec<&str> = project_out
            .iter()
            .map(|view| match view.get("name") {
                Some(ScalarRef::Text(name)) => name,
                other => panic!("expected Text, found {other:?}"),
            })
            .collect();
        names.sort_unstable();
        assert_eq!(names, ["Alice", "Dave", "Grace", "Judy"]);
    }
}
