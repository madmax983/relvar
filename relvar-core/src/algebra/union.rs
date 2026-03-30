//! Union operator for combining relations.
//!
//! The union operator combines all tuples from two type-compatible relations
//! into a single relation. As required by set semantics, duplicate tuples
//! are automatically eliminated.
//!
//! # TTM Compliance
//!
//! - Relations must be type-compatible (identical headings)
//! - Duplicates are automatically eliminated (set semantics)
//! - Result is a valid relation
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
//! let rel_type = RelationType::new(heading);
//!
//! let mut rel1 = Relation::new(rel_type.clone());
//! rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
//!
//! let mut rel2 = Relation::new(rel_type);
//! rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
//!
//! let result = rel1.union(&rel2).unwrap();
//! assert_eq!(result.cardinality(), 2);  // Alice and Bob
//! ```

use crate::values::Relation;
use thiserror::Error;

/// Errors that can occur during union operations.
#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
#[error("Relations must have the same type (heading) for union")]
pub struct UnionError;

impl Relation {
    /// Computes the union of this relation with another.
    ///
    /// This is the set union operator from relational algebra. It produces a
    /// new relation containing all tuples that appear in either relation.
    /// Duplicate tuples are automatically eliminated per set semantics.
    ///
    /// # Arguments
    ///
    /// * `other` - The relation to union with. Must have the same heading
    ///   (type) as this relation.
    ///
    /// # Returns
    ///
    /// A new relation containing all tuples from both relations, with
    /// duplicates removed.
    ///
    /// # Errors
    ///
    /// Returns [`UnionError`] if the relations have different
    /// headings (different attribute names or types).
    ///
    /// # Behavior
    ///
    /// - Both relations must be type-compatible (identical headings)
    /// - Duplicate tuples across relations are eliminated
    /// - Union with self returns an equivalent relation
    /// - Union with empty relation returns the non-empty relation
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("emp_id", ScalarType::Int)
    ///     .with_attribute("name", ScalarType::String);
    ///
    /// let rel_type = RelationType::new(heading);
    ///
    /// let mut rel1 = Relation::new(rel_type.clone());
    /// rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
    /// rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
    ///
    /// let mut rel2 = Relation::new(rel_type);
    /// rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();  // Duplicate
    /// rel2.insert(tuple! { emp_id: 3i64, name: "Charlie" }).unwrap();
    ///
    /// let result = rel1.union(&rel2).unwrap();
    /// assert_eq!(result.cardinality(), 3);  // Alice, Bob (once), Charlie
    /// ```
    pub fn union(&self, other: &Relation) -> Result<Self, UnionError> {
        // Check type compatibility
        if self.relation_type() != other.relation_type() {
            return Err(UnionError);
        }

        // Chain iterators and create relation without redundant checks
        // Safety: both relations are verified to have the same type, so their tuples must conform
        Ok(Relation::from_tuples_unchecked(
            self.relation_type().clone(),
            self.tuples().chain(other.tuples()).cloned(),
        ))
    }

    /// Computes the union of this relation with another, consuming this relation.
    ///
    /// This is an optimized version of `union` that avoids O(N) tuple clones for the
    /// first relation by consuming it.
    pub fn union_into(mut self, other: &Relation) -> Result<Self, UnionError> {
        // Check type compatibility
        if self.relation_type() != other.relation_type() {
            return Err(UnionError);
        }

        for tuple in other.tuples() {
            self.insert(tuple.clone()).unwrap();
        }

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    fn emp_type() -> TupleType {
        TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
    }

    #[test]
    fn test_union_requires_type_compatible_relations() {
        let type1 = emp_type();
        let rel_type1 = RelationType::new(type1);
        let rel1 = Relation::new(rel_type1);

        let type2 = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("salary", ScalarType::Float);

        let rel_type2 = RelationType::new(type2);
        let rel2 = Relation::new(rel_type2);

        let result = rel1.union(&rel2);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), UnionError);
    }

    #[test]
    fn test_union_removes_duplicates() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap(); // Duplicate
        rel2.insert(tuple! { emp_id: 3i64, name: "Charlie" })
            .unwrap();

        let result = rel1.union(&rel2).unwrap();

        assert_eq!(result.cardinality(), 3); // Alice, Bob, Charlie (Bob not duplicated)

        let alice = tuple! { emp_id: 1i64, name: "Alice" };
        let bob = tuple! { emp_id: 2i64, name: "Bob" };
        let charlie = tuple! { emp_id: 3i64, name: "Charlie" };

        assert!(result.contains(&alice));
        assert!(result.contains(&bob));
        assert!(result.contains(&charlie));
    }

    #[test]
    fn test_union_with_empty_relation() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let rel2 = Relation::new(rel_type);

        let result = rel1.union(&rel2).unwrap();
        assert_eq!(result.cardinality(), 1);
        assert_eq!(result, rel1);
    }

    #[test]
    fn test_union_of_disjoint_relations() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let result = rel1.union(&rel2).unwrap();
        assert_eq!(result.cardinality(), 2);
    }

    #[test]
    fn test_union_identical_relations() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let result = rel1.union(&rel1).unwrap();
        assert_eq!(result.cardinality(), 2); // No duplicates
        assert_eq!(result, rel1);
    }

    #[test]
    fn test_union_into_removes_duplicates() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap(); // Duplicate
        rel2.insert(tuple! { emp_id: 3i64, name: "Charlie" })
            .unwrap();

        let result = rel1.union_into(&rel2).unwrap();

        assert_eq!(result.cardinality(), 3); // Alice, Bob, Charlie (Bob not duplicated)

        let alice = tuple! { emp_id: 1i64, name: "Alice" };
        let bob = tuple! { emp_id: 2i64, name: "Bob" };
        let charlie = tuple! { emp_id: 3i64, name: "Charlie" };

        assert!(result.contains(&alice));
        assert!(result.contains(&bob));
        assert!(result.contains(&charlie));
    }
}
