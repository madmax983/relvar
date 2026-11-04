//! Relation values for the relational model.
//!
//! A relation is a set of tuples that all conform to the same relation type.
//! This is the fundamental data structure in the relational model.
//!
//! # TTM Compliance
//!
//! - **Proscription 2**: No duplicate tuples (relations are true sets)
//! - **Proscription 3**: No tuple ordering (tuples are unordered)
//! - All tuples conform to the relation's heading
//! - Relation equality is based on heading and body content
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::Relation;
//! use relvar_core::tuple;
//!
//! let heading = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//!
//! let mut employees = Relation::new(RelationType::new(heading));
//!
//! // Insert tuples
//! employees.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
//! employees.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
//!
//! // Duplicates are silently ignored (set semantics)
//! employees.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
//!
//! assert_eq!(employees.cardinality(), 2);  // Still 2, not 3
//! ```

use crate::collections::HashSet;
use crate::types::{RelationType, TupleType};
use crate::values::Tuple;
use alloc::sync::Arc;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when working with relations.
///
/// This enum represents all possible ways a relational operation might fail.
///
/// # Examples
///
/// ```rust
/// use relvar_core::values::RelationError;
///
/// // This error occurs when a tuple does not match a relation's heading.
/// let error = RelationError;
/// ```
#[derive(Debug, Error)]
#[error("Tuple does not conform to relation type")]
pub struct RelationError;

/// A relation value consisting of a heading and a body.
///
/// A relation has two components:
///
/// 1. **Heading** (relation type) - Defines the structure (attributes and types)
/// 2. **Body** - A set of tuples that conform to the heading
///
/// # Set Semantics
///
/// Per TTM Proscription 2, the body is a true mathematical set:
///
/// - No duplicate tuples are permitted
/// - Inserting a duplicate has no effect
/// - There is no inherent ordering of tuples
///
/// # Type Safety
///
/// All tuples in a relation must conform to the relation's heading. Attempting
/// to insert a tuple with a different structure results in an error.
///
/// # Examples
///
/// ```
/// use relvar_core::types::{TupleType, RelationType, ScalarType};
/// use relvar_core::values::Relation;
/// use relvar_core::tuple;
///
/// // Define the relation type
/// let heading = TupleType::new()
///     .with_attribute("id", ScalarType::Int)
///     .with_attribute("name", ScalarType::String);
///
/// let rel_type = RelationType::new(heading);
///
/// // Create an empty relation
/// let mut relation = Relation::new(rel_type);
/// assert!(relation.is_empty());
///
/// // Insert tuples
/// relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
/// relation.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
///
/// // Query the relation
/// assert_eq!(relation.cardinality(), 2);
/// assert_eq!(relation.degree(), 2);
///
/// // Check for tuple membership
/// let alice = tuple! { id: 1i64, name: "Alice" };
/// assert!(relation.contains(&alice));
/// ```
///
/// # Relational Algebra
///
/// The `Relation` struct implements the full suite of relational algebra operators
/// as inherent methods. These methods transform relations into new relations
/// according to TTM principles.
///
/// Available operators include:
///
/// - **Selection**: [`restrict`](Relation::restrict)
/// - **Projection**: [`project`](Relation::project)
/// - **Renaming**: [`rename`](Relation::rename)
/// - **Set Operations**: [`union`](Relation::union), [`intersect`](Relation::intersect), [`difference`](Relation::difference)
/// - **Joins**: [`join`](Relation::join) (natural), [`theta_join`](Relation::theta_join), [`semijoin`](Relation::semijoin)
/// - **Advanced**: [`extend`](Relation::extend), [`summarize`](Relation::summarize), [`group`](Relation::group), [`ungroup`](Relation::ungroup)
/// - **Division**: [`divide`](Relation::divide)
///
/// See the [`algebra`](crate::algebra) module for detailed documentation on each operator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RelationUnchecked")]
pub struct Relation {
    /// The relation type (heading) that defines the structure.
    relation_type: RelationType,
    /// The body: a set of tuples conforming to the heading.
    pub(crate) body: HashSet<Tuple>,
}

/// Wire shape of [`Relation`]: decoded first, then validated by `TryFrom` so
/// a deserialized relation upholds the invariant [`Relation::insert`] does.
/// Field names match [`Relation`]'s, so the serialized format is unchanged.
/// (Each [`Tuple`] is itself validated against its own heading while
/// decoding; duplicate tuples collapse into the set.)
#[derive(Deserialize)]
struct RelationUnchecked {
    relation_type: RelationType,
    body: HashSet<Tuple>,
}

impl TryFrom<RelationUnchecked> for Relation {
    type Error = RelationError;

    /// Accepts the decoded relation only if every tuple of the body has
    /// exactly the relation's heading.
    fn try_from(unchecked: RelationUnchecked) -> Result<Self, Self::Error> {
        let heading = unchecked.relation_type.heading();
        if unchecked
            .body
            .iter()
            .any(|tuple| tuple.tuple_type() != heading)
        {
            return Err(RelationError);
        }
        Ok(Self {
            relation_type: unchecked.relation_type,
            body: unchecked.body,
        })
    }
}

// Custom Hash implementation for Relation
// Since HashSet doesn't have a deterministic iteration order, we need to
// hash the relation in a way that's independent of the set's internal ordering
impl core::hash::Hash for Relation {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Hash the relation type
        self.relation_type.hash(state);

        // Hash the cardinality
        self.body.len().hash(state);

        // Hash all tuples in a deterministic way by XORing their hashes
        // This works because XOR is commutative and associative
        let mut combined_hash = 0u64;
        for tuple in &self.body {
            let mut hasher = crate::collections::new_hasher();
            tuple.hash(&mut hasher);
            combined_hash ^= core::hash::Hasher::finish(&hasher);
        }
        combined_hash.hash(state);
    }
}

impl PartialOrd for Relation {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A deterministic total order on relation values, consistent with `Eq`
/// (`cmp` is `Equal` exactly when the relations are equal).
///
/// Relations are ordered by cardinality, then heading (see [`TupleType`]'s
/// `Ord`), then by their bodies compared as sorted tuple sequences (see
/// [`Tuple`]'s `Ord`). The order carries no relational meaning — bodies are
/// unordered sets (Proscription 3) — it exists so relation values can be
/// keys of sorted containers and sorted canonically.
///
/// # Performance
///
/// Equal relations are detected by set equality in `O(n)`; ordering two
/// unequal relations of the same cardinality and heading sorts both bodies,
/// `O(n log n)` with one temporary allocation per side.
impl Ord for Relation {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        use core::cmp::Ordering;

        self.cardinality()
            .cmp(&other.cardinality())
            .then_with(|| self.relation_type.cmp(&other.relation_type))
            .then_with(|| {
                if self.body == other.body {
                    return Ordering::Equal;
                }
                let mut ours: Vec<&Tuple> = self.body.iter().collect();
                let mut theirs: Vec<&Tuple> = other.body.iter().collect();
                ours.sort_unstable();
                theirs.sort_unstable();
                ours.cmp(&theirs)
            })
    }
}

impl IntoIterator for Relation {
    type Item = Tuple;
    type IntoIter = hashbrown::hash_set::IntoIter<Tuple>;

    fn into_iter(self) -> Self::IntoIter {
        self.body.into_iter()
    }
}

impl Relation {
    /// Creates a new empty relation with the given type.
    ///
    /// The relation starts with zero tuples but has a defined structure
    /// (heading) that all future tuples must conform to.
    ///
    /// # Arguments
    ///
    /// * `relation_type` - The type defining the relation's structure
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let relation = Relation::new(RelationType::new(heading));
    /// assert!(relation.is_empty());
    /// assert_eq!(relation.degree(), 1);
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let relation = Relation::new(RelationType::new(heading));
    /// assert!(relation.is_empty());
    /// ```
    pub fn new(relation_type: RelationType) -> Self {
        Self {
            relation_type,
            body: HashSet::new(),
        }
    }

    /// Creates a new empty relation with the given type and initial capacity.
    ///
    /// # Arguments
    ///
    /// * `relation_type` - The type defining the relation's structure
    /// * `capacity` - The initial capacity of the underlying storage
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let relation = Relation::with_capacity(RelationType::new(heading), 10);
    /// assert!(relation.is_empty());
    /// ```
    pub fn with_capacity(relation_type: RelationType, capacity: usize) -> Self {
        Self {
            relation_type,
            body: HashSet::with_capacity(capacity),
        }
    }

    /// Creates a relation from a type and an iterable of tuples.
    ///
    /// This is useful for creating a pre-populated relation. Duplicate
    /// tuples in the input are automatically removed per set semantics.
    ///
    /// # Arguments
    ///
    /// * `relation_type` - The type defining the relation's structure
    /// * `tuples` - An iterable of tuples to populate the relation
    ///
    /// # Errors
    ///
    /// Returns [`RelationError`] if any tuple doesn't conform
    /// to the relation type.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let tuples = vec![
    ///     tuple! { id: 1i64 },
    ///     tuple! { id: 2i64 },
    ///     tuple! { id: 1i64 },  // Duplicate - will be removed
    /// ];
    ///
    /// let relation = Relation::from_tuples(RelationType::new(heading), tuples).unwrap();
    /// assert_eq!(relation.cardinality(), 2);  // Only 2 unique tuples
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::{Relation, Tuple};
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    ///
    /// let tuples = vec![tuple!{ id: 1i64 }];
    /// let relation = Relation::from_tuples(rel_type, tuples).unwrap();
    /// assert_eq!(relation.cardinality(), 1);
    /// ```
    pub fn from_tuples(
        relation_type: RelationType,
        tuples: impl IntoIterator<Item = Tuple>,
    ) -> Result<Self, RelationError> {
        let iter = tuples.into_iter();
        let (lower, _upper) = iter.size_hint();
        // Use lower bound as a conservative estimate to avoid over-allocation if upper is None or very large
        let mut body = HashSet::with_capacity(lower);

        let expected_heading = relation_type.heading();

        // Optimization: tuples built from one shared `Arc<TupleType>` (the
        // common case) are checked by pointer identity instead of a deep
        // heading comparison. The cache *holds* a clone of the last validated
        // `Arc`, so that heading stays alive and its address can never be
        // freed and reused by a different heading (e.g. after a duplicate
        // tuple — and its sole `Arc` — is dropped by the set below).
        let mut last_checked: Option<Arc<TupleType>> = None;

        for tuple in iter {
            let current = tuple.tuple_type_arc();
            let cached = last_checked
                .as_ref()
                .is_some_and(|checked| Arc::ptr_eq(checked, current));

            if !cached {
                // Slow path: full structural comparison.
                if current.as_ref() != expected_heading {
                    return Err(RelationError);
                }
                last_checked = Some(Arc::clone(current));
            }

            body.insert(tuple);
        }

        Ok(Self {
            relation_type,
            body,
        })
    }

    /// Creates a relation from a type and an iterable of tuples without type checking.
    ///
    /// # Safety
    ///
    /// This method bypasses the check that ensures all tuples conform to the relation type.
    /// The caller must guarantee that all tuples in the iterator strictly conform to
    /// `relation_type.heading()`.
    ///
    /// It still performs duplicate elimination (set semantics).
    /// Creates a new relation from an iterator of tuples without type validation.
    ///
    /// # Performance
    ///
    /// This method pre-allocates the underlying `HashSet` based on the iterator's
    /// size hint. It prefers the upper bound (`upper`) over the lower bound (`lower`).
    /// This is a critical optimization for chained iterators (e.g., those using `.filter()`),
    /// which typically yield a lower bound of `0`. By falling back to the upper bound,
    /// we avoid zero-capacity allocations and the significant internal reallocation
    /// churn that would occur as tuples are inserted.
    pub(crate) fn from_tuples_unchecked(
        relation_type: RelationType,
        tuples: impl IntoIterator<Item = Tuple>,
    ) -> Self {
        let iter = tuples.into_iter();
        let (lower, upper) = iter.size_hint();
        let capacity = upper.unwrap_or(lower);
        let mut body = HashSet::with_capacity(capacity);

        for tuple in iter {
            body.insert(tuple);
        }

        Self {
            relation_type,
            body,
        }
    }

    /// Creates a new relation directly from a `HashSet` of tuples without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure that all provided tuples perfectly match the
    /// `relation_type`'s heading (same attribute names and types).
    pub(crate) fn from_body_unchecked(relation_type: RelationType, body: HashSet<Tuple>) -> Self {
        Self {
            relation_type,
            body,
        }
    }

    /// Retrieves the relation type (heading) defining this relation's structure.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let rel_type = RelationType::new(heading.clone());
    /// let relation = Relation::new(rel_type);
    ///
    /// assert_eq!(relation.relation_type().heading(), &heading);
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// let relation = Relation::new(rel_type.clone());
    ///
    /// assert_eq!(relation.relation_type(), &rel_type);
    /// ```
    pub fn relation_type(&self) -> &RelationType {
        &self.relation_type
    }

    /// Calculates the cardinality (total number of tuples) currently held in this relation.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let mut relation = Relation::new(RelationType::new(heading));
    /// assert_eq!(relation.cardinality(), 0);
    ///
    /// relation.insert(tuple! { id: 1i64 }).unwrap();
    /// assert_eq!(relation.cardinality(), 1);
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let mut relation = Relation::new(RelationType::new(heading));
    ///
    /// relation.insert(tuple!{ id: 1i64 }).unwrap();
    /// assert_eq!(relation.cardinality(), 1);
    /// ```
    pub fn cardinality(&self) -> usize {
        self.body.len()
    }

    /// Calculates the degree (number of attributes) defined by this relation's heading.
    ///
    /// This is determined by the relation type's heading.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int)
    ///     .with_attribute("name", ScalarType::String);
    ///
    /// let relation = Relation::new(RelationType::new(heading));
    /// assert_eq!(relation.degree(), 2);
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int)
    ///     .with_attribute("name", ScalarType::String);
    /// let relation = Relation::new(RelationType::new(heading));
    ///
    /// assert_eq!(relation.degree(), 2);
    /// ```
    pub fn degree(&self) -> usize {
        self.relation_type.degree()
    }

    /// Inserts a tuple into the relation.
    ///
    /// Returns `Ok(true)` if the tuple was inserted, or `Ok(false)` if the
    /// tuple was already present (duplicates are silently ignored per set
    /// semantics).
    ///
    /// # Arguments
    ///
    /// * `tuple` - The tuple to insert (must conform to the relation type)
    ///
    /// # Errors
    ///
    /// Returns [`RelationError`] if the tuple doesn't conform
    /// to the relation's type.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let mut relation = Relation::new(RelationType::new(heading));
    ///
    /// assert!(relation.insert(tuple! { id: 1i64 }).unwrap());  // true: inserted
    /// assert!(!relation.insert(tuple! { id: 1i64 }).unwrap()); // false: duplicate
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let mut relation = Relation::new(RelationType::new(heading));
    ///
    /// let was_inserted = relation.insert(tuple!{ id: 1i64 }).unwrap();
    /// assert!(was_inserted);
    ///
    /// // Sets do not allow duplicates
    /// let was_inserted_again = relation.insert(tuple!{ id: 1i64 }).unwrap();
    /// assert!(!was_inserted_again);
    /// ```
    pub fn insert(&mut self, tuple: Tuple) -> Result<bool, RelationError> {
        // Verify tuple conforms to the relation type
        if tuple.tuple_type() != self.relation_type.heading() {
            return Err(RelationError);
        }

        Ok(self.body.insert(tuple))
    }

    /// Checks if the relation contains a specific tuple.
    ///
    /// # Arguments
    ///
    /// * `tuple` - The tuple to search for
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let mut relation = Relation::new(RelationType::new(heading));
    /// relation.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// assert!(relation.contains(&tuple! { id: 1i64 }));
    /// assert!(!relation.contains(&tuple! { id: 2i64 }));
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let mut relation = Relation::new(RelationType::new(heading));
    ///
    /// let t = tuple!{ id: 1i64 };
    /// relation.insert(t.clone()).unwrap();
    /// assert!(relation.contains(&t));
    /// ```
    pub fn contains(&self, tuple: &Tuple) -> bool {
        self.body.contains(tuple)
    }

    /// Provides an iterator yielding all tuples currently in the relation.
    ///
    /// Per TTM Proscription 3, tuples have no inherent ordering. The
    /// iteration order is not guaranteed to be consistent.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let mut relation = Relation::new(RelationType::new(heading));
    /// relation.insert(tuple! { id: 1i64 }).unwrap();
    /// relation.insert(tuple! { id: 2i64 }).unwrap();
    ///
    /// for tuple in relation.tuples() {
    ///     println!("{:?}", tuple);
    /// }
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let mut relation = Relation::new(RelationType::new(heading));
    /// relation.insert(tuple!{ id: 1i64 }).unwrap();
    ///
    /// let all_tuples: Vec<_> = relation.tuples().collect();
    /// assert_eq!(all_tuples.len(), 1);
    /// ```
    pub fn tuples(&self) -> impl Iterator<Item = &Tuple> {
        self.body.iter()
    }

    /// Checks if the relation has no tuples.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let mut relation = Relation::new(RelationType::new(heading));
    /// assert!(relation.is_empty());
    ///
    /// relation.insert(tuple! { id: 1i64 }).unwrap();
    /// assert!(!relation.is_empty());
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let relation = Relation::new(RelationType::new(heading));
    /// assert!(relation.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.body.is_empty()
    }

    /// Restricts this relation by filtering tuples in-place.
    ///
    /// This consumes the relation and modifies it, avoiding allocations for a new relation
    /// and avoiding cloning tuples.
    ///
    /// # Arguments
    ///
    /// * `predicate` - A function that returns `true` for tuples to keep
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let mut relation = Relation::new(RelationType::new(heading));
    /// relation.insert(tuple!{ id: 1i64 }).unwrap();
    /// relation.insert(tuple!{ id: 2i64 }).unwrap();
    ///
    /// let evens = relation.restrict_into(|t| t.get_typed::<i64>("id").unwrap() % 2 == 0);
    /// assert_eq!(evens.cardinality(), 1);
    /// ```
    pub fn restrict_into<F>(mut self, mut predicate: F) -> Self
    where
        F: FnMut(&Tuple) -> bool,
    {
        self.body.retain(|tuple| predicate(tuple));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{ScalarType, TupleType};
    use crate::values::ScalarValue;
    use alloc::collections::BTreeMap;
    use alloc::string::ToString;

    fn emp_type() -> TupleType {
        TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
    }

    #[test]
    fn test_relation_heading_is_tuple_type() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading.clone());
        let relation = Relation::new(rel_type);

        assert_eq!(relation.relation_type().heading(), &heading);
    }

    #[test]
    fn test_relation_body_is_set_no_duplicates() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        let tuple1 = tuple! { emp_id: 1i64, name: "Alice" };
        let tuple2 = tuple! { emp_id: 1i64, name: "Alice" }; // Duplicate

        assert!(relation.insert(tuple1).unwrap());
        assert!(!relation.insert(tuple2).unwrap()); // Returns false for duplicate

        assert_eq!(relation.cardinality(), 1);
    }

    #[test]
    fn test_relation_equality() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap(); // Different order
        rel2.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        assert_eq!(rel1, rel2);
    }

    #[test]
    fn test_cardinality_and_degree() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        assert_eq!(relation.cardinality(), 0);
        assert_eq!(relation.degree(), 2);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob" })
            .unwrap();

        assert_eq!(relation.cardinality(), 2);
        assert_eq!(relation.degree(), 2);
    }

    #[test]
    fn test_insert_type_mismatch() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        // Wrong type
        let wrong_tuple = tuple! { emp_id: 1i64, salary: 50000.0 };

        let result = relation.insert(wrong_tuple);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RelationError));
    }

    #[test]
    fn test_from_tuples() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let tuples = vec![
            tuple! { emp_id: 1i64, name: "Alice" },
            tuple! { emp_id: 2i64, name: "Bob" },
            tuple! { emp_id: 1i64, name: "Alice" }, // Duplicate
        ];

        let relation = Relation::from_tuples(rel_type, tuples).unwrap();

        assert_eq!(relation.cardinality(), 2); // Duplicate removed
    }

    #[test]
    fn test_contains() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        let tuple1 = tuple! { emp_id: 1i64, name: "Alice" };
        let tuple2 = tuple! { emp_id: 2i64, name: "Bob" };

        relation.insert(tuple1.clone()).unwrap();

        assert!(relation.contains(&tuple1));
        assert!(!relation.contains(&tuple2));
    }

    #[test]
    fn should_create_relation_with_capacity() {
        let rel_type = RelationType::new(TupleType::new());
        let capacity = 100;
        let relation = Relation::with_capacity(rel_type, capacity);

        assert!(relation.is_empty());
        assert_eq!(relation.degree(), 0);
        assert!(relation.body.capacity() >= capacity);
    }

    #[test]
    fn test_empty_relation() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        assert!(relation.is_empty());
        assert_eq!(relation.cardinality(), 0);
    }

    #[test]
    fn test_tuples_iterator() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob" })
            .unwrap();

        let count = relation.tuples().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_from_tuples_optimization_correctness() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading.clone());

        // Create a valid tuple
        let valid_tuple = tuple! { emp_id: 1i64, name: "Alice" };

        // Create an invalid tuple (wrong type)
        // Note: tuple! macro creates a tuple with inferred type from values.
        // So this tuple has a different TupleType than 'heading'.
        let invalid_tuple = tuple! { emp_id: 2i64, name: 12345 }; // Name is Int, expected String

        // Case 1: All valid (optimization path should work)
        let tuples = vec![valid_tuple.clone(), valid_tuple.clone()];
        let rel = Relation::from_tuples(rel_type.clone(), tuples);
        assert!(rel.is_ok());

        // Case 2: Mix valid and invalid (optimization should not hide error)
        // The first tuple sets the cached valid pointer.
        // The second tuple has a different pointer (and type), so it should be checked and fail.
        let tuples = vec![valid_tuple.clone(), invalid_tuple.clone()];
        let rel = Relation::from_tuples(rel_type.clone(), tuples);
        assert!(rel.is_err());
        assert!(matches!(rel.unwrap_err(), RelationError));

        // Case 3: Invalid first
        // The first tuple fails immediately.
        let tuples = vec![invalid_tuple.clone(), valid_tuple.clone()];
        let rel = Relation::from_tuples(rel_type.clone(), tuples);
        assert!(rel.is_err());
    }

    /// Regression: the heading-check cache in `from_tuples` must not be keyed
    /// on a raw address. When a duplicate tuple is dropped, its (sole)
    /// `Arc<TupleType>` is freed and the allocator may hand the same address
    /// to the *next* tuple's — different — heading. That tuple must still be
    /// rejected.
    #[test]
    fn test_from_tuples_rejects_wrong_heading_after_freed_duplicate_heading() {
        let heading = emp_type();
        let wrong_heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::Int);

        let make = |heading: &TupleType, id: i64, name: ScalarValue| {
            let mut values = BTreeMap::new();
            values.insert("emp_id".to_string(), ScalarValue::Int(id));
            values.insert("name".to_string(), name);
            // A fresh, unshared Arc per tuple: dropping the tuple frees it.
            Tuple::new(Arc::new(heading.clone()), values).unwrap()
        };

        // Repeat so an allocator that does not reuse the slot on the first
        // try still gets plenty of chances; every attempt must be rejected.
        for _ in 0..64 {
            let mut step = 0;
            let tuples = core::iter::from_fn(|| {
                step += 1;
                match step {
                    1 => Some(make(&heading, 1, ScalarValue::String("Alice".into()))),
                    // Duplicate of #1 with its own heading Arc: checked,
                    // cached, then dropped by the set (freeing the Arc).
                    2 => Some(make(&heading, 1, ScalarValue::String("Alice".into()))),
                    // Different heading, built lazily after #2 was dropped.
                    3 => Some(make(&wrong_heading, 2, ScalarValue::Int(7))),
                    _ => None,
                }
            });
            let result = Relation::from_tuples(RelationType::new(heading.clone()), tuples);
            assert!(
                result.is_err(),
                "a tuple whose heading differs from the relation's was accepted"
            );
        }
    }
}

/// Deserialization must uphold the same invariant as [`Relation::insert`]:
/// every tuple of the body has exactly the relation's heading.
#[cfg(test)]
mod deserialize_validation_tests {
    use super::*;
    use crate::tuple;
    use crate::types::ScalarType;
    use crate::values::ScalarValue;

    fn id_type() -> RelationType {
        RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int))
    }

    /// A relation whose body bypassed the heading check, i.e. what a
    /// corrupted or hostile serialized payload decodes to.
    fn forged(tuples: impl IntoIterator<Item = Tuple>) -> Relation {
        Relation::from_tuples_unchecked(id_type(), tuples)
    }

    fn assert_rejected(forged: &Relation) {
        let json = serde_json::to_string(forged).unwrap();
        assert!(
            serde_json::from_str::<Relation>(&json).is_err(),
            "JSON accepted a non-conforming relation: {json}"
        );
        let bytes = postcard::to_allocvec(forged).unwrap();
        assert!(
            postcard::from_bytes::<Relation>(&bytes).is_err(),
            "postcard accepted a non-conforming relation"
        );
    }

    #[test]
    fn deserialize_round_trips_valid_relation() {
        let rel =
            Relation::from_tuples(id_type(), [tuple! { id: 1i64 }, tuple! { id: 2i64 }]).unwrap();
        let json = serde_json::to_string(&rel).unwrap();
        assert_eq!(serde_json::from_str::<Relation>(&json).unwrap(), rel);
        let bytes = postcard::to_allocvec(&rel).unwrap();
        assert_eq!(postcard::from_bytes::<Relation>(&bytes).unwrap(), rel);
    }

    #[test]
    fn deserialize_rejects_tuple_of_another_heading() {
        assert_rejected(&forged([tuple! { id: 1i64 }, tuple! { name: "x" }]));
    }

    #[test]
    fn deserialize_rejects_tuple_with_wider_heading() {
        assert_rejected(&forged([tuple! { id: 1i64, x: 2i64 }]));
    }

    #[test]
    fn deserialize_rejects_tuple_with_attribute_of_other_type() {
        assert_rejected(&forged([tuple! { id: "one" }]));
    }

    #[test]
    fn deserialize_rejects_nested_relation_value_with_foreign_tuple() {
        // An RVA value smuggling a foreign tuple is rejected too: the nested
        // relation goes through the same validation.
        let value = ScalarValue::Relation(forged([tuple! { id: "one" }]));
        let json = serde_json::to_string(&value).unwrap();
        assert!(serde_json::from_str::<ScalarValue>(&json).is_err());
    }
}
