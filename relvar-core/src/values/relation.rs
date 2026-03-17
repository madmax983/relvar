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

use crate::types::{RelationType, TupleType};
use crate::values::Tuple;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Errors that can occur when working with relations.
#[derive(Debug, Error)]
pub enum RelationError {
    /// A tuple doesn't conform to the relation's type (heading).
    ///
    /// This occurs when attempting to insert a tuple with a different
    /// structure (different attribute names or types) than what the
    /// relation expects. The tuple must have exactly the same attributes
    /// with the same types as defined in the relation's heading.
    #[error("Tuple does not conform to relation type")]
    TypeMismatch,

    /// A duplicate tuple was detected.
    ///
    /// This error is used in contexts where uniqueness is strictly enforced
    /// and silent deduplication is not desired (e.g., bulk loading with strict validation).
    /// Standard insert operations typically return `Ok(false)` for duplicates
    /// rather than this error, adhering to set semantics.
    #[error("Duplicate tuple")]
    DuplicateTuple,
}

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
/// # Example
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
pub struct Relation {
    /// The relation type (heading) that defines the structure.
    relation_type: RelationType,
    /// The body: a set of tuples conforming to the heading.
    body: HashSet<Tuple>,
}

// Custom Hash implementation for Relation
// Since HashSet doesn't have a deterministic iteration order, we need to
// hash the relation in a way that's independent of the set's internal ordering
impl std::hash::Hash for Relation {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the relation type
        self.relation_type.hash(state);

        // Hash the cardinality
        self.body.len().hash(state);

        // Hash all tuples in a deterministic way by XORing their hashes
        // This works because XOR is commutative and associative
        let mut combined_hash = 0u64;
        for tuple in &self.body {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            tuple.hash(&mut hasher);
            combined_hash ^= std::hash::Hasher::finish(&hasher);
        }
        combined_hash.hash(state);
    }
}

impl IntoIterator for Relation {
    type Item = Tuple;
    type IntoIter = std::collections::hash_set::IntoIter<Tuple>;

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
    /// # Example
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
    /// Returns [`RelationError::TypeMismatch`] if any tuple doesn't conform
    /// to the relation type.
    ///
    /// # Example
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
    pub fn from_tuples(
        relation_type: RelationType,
        tuples: impl IntoIterator<Item = Tuple>,
    ) -> Result<Self, RelationError> {
        let iter = tuples.into_iter();
        let (lower, _upper) = iter.size_hint();
        // Use lower bound as a conservative estimate to avoid over-allocation if upper is None or very large
        let mut body = HashSet::with_capacity(lower);

        let expected_heading = relation_type.heading();

        // Optimization: Cache pointer to last checked TupleType to avoid repeated deep comparisons
        // when inserting many tuples that share the same Arc<TupleType> (common case).
        let mut last_checked_type_ptr: Option<*const TupleType> = None;

        for tuple in iter {
            let current_type_ref = tuple.tuple_type();
            let current_type_ptr = current_type_ref as *const TupleType;

            // Fast path: if pointer is same as last checked (which we know is valid), skip check
            if Some(current_type_ptr) != last_checked_type_ptr {
                // Slow path: full comparison
                if current_type_ref != expected_heading {
                    return Err(RelationError::TypeMismatch);
                }
                // If match, update cache
                last_checked_type_ptr = Some(current_type_ptr);
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
    pub(crate) fn from_tuples_unchecked(
        relation_type: RelationType,
        tuples: impl IntoIterator<Item = Tuple>,
    ) -> Self {
        let iter = tuples.into_iter();
        let (lower, _upper) = iter.size_hint();
        let mut body = HashSet::with_capacity(lower);

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
    /// # Example
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
    pub fn relation_type(&self) -> &RelationType {
        &self.relation_type
    }

    /// Calculates the cardinality (total number of tuples) currently held in this relation.
    ///
    /// # Example
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
    pub fn cardinality(&self) -> usize {
        self.body.len()
    }

    /// Calculates the degree (number of attributes) defined by this relation's heading.
    ///
    /// This is determined by the relation type's heading.
    ///
    /// # Example
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
    /// Returns [`RelationError::TypeMismatch`] if the tuple doesn't conform
    /// to the relation's type.
    ///
    /// # Example
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
    pub fn insert(&mut self, tuple: Tuple) -> Result<bool, RelationError> {
        // Verify tuple conforms to the relation type
        if tuple.tuple_type() != self.relation_type.heading() {
            return Err(RelationError::TypeMismatch);
        }

        Ok(self.body.insert(tuple))
    }

    /// Checks if the relation contains a specific tuple.
    ///
    /// # Arguments
    ///
    /// * `tuple` - The tuple to search for
    ///
    /// # Example
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
    pub fn contains(&self, tuple: &Tuple) -> bool {
        self.body.contains(tuple)
    }

    /// Provides an iterator yielding all tuples currently in the relation.
    ///
    /// Per TTM Proscription 3, tuples have no inherent ordering. The
    /// iteration order is not guaranteed to be consistent.
    ///
    /// # Example
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
    pub fn tuples(&self) -> impl Iterator<Item = &Tuple> {
        self.body.iter()
    }

    /// Checks if the relation has no tuples.
    ///
    /// # Example
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
        assert!(matches!(result.unwrap_err(), RelationError::TypeMismatch));
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
        assert!(matches!(rel.unwrap_err(), RelationError::TypeMismatch));

        // Case 3: Invalid first
        // The first tuple fails immediately.
        let tuples = vec![invalid_tuple.clone(), valid_tuple.clone()];
        let rel = Relation::from_tuples(rel_type.clone(), tuples);
        assert!(rel.is_err());
    }
}
