//! Intersect operator for finding common tuples.
//!
//! The intersection operator returns only the tuples that appear in both
//! of two type-compatible relations.
//!
//! # TTM Compliance
//!
//! - Relations must be type-compatible (identical headings)
//! - Result contains only tuples present in both relations
//! - Set semantics are maintained
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
//! rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
//!
//! let mut rel2 = Relation::new(rel_type);
//! rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
//! rel2.insert(tuple! { emp_id: 3i64, name: "Charlie" }).unwrap();
//!
//! let result = rel1.intersect(&rel2).unwrap();
//! assert_eq!(result.cardinality(), 1);  // Only Bob is in both
//! ```

use crate::values::Relation;
use thiserror::Error;

/// Errors that can occur during intersection operations.
#[derive(Debug, Error)]
#[error("Relations must have the same type (heading) for intersection")]
pub struct IntersectError;

impl Relation {
    /// Computes the intersection of this relation with another.
    ///
    /// This is the set intersection operator from relational algebra. It
    /// produces a new relation containing only the tuples that appear in
    /// both relations.
    ///
    /// # Arguments
    ///
    /// * `other` - The relation to intersect with. Must have the same heading
    ///   (type) as this relation.
    ///
    /// # Returns
    ///
    /// A new relation containing only tuples present in both relations.
    ///
    /// # Errors
    ///
    /// Returns [`IntersectError`] if the relations have different
    /// headings (different attribute names or types).
    ///
    /// # Behavior
    ///
    /// - Both relations must be type-compatible (identical headings)
    /// - Only tuples present in both relations are included
    /// - Intersection with self returns an equivalent relation
    /// - Intersection with empty relation returns empty relation
    /// - If no common tuples exist, returns empty relation
    ///
    /// # Complexity
    ///
    /// O(n * m) where n and m are the cardinalities of the two relations,
    /// due to tuple membership testing.
    ///
    /// # Examples
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
    /// let mut current_employees = Relation::new(rel_type.clone());
    /// current_employees.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
    /// current_employees.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
    ///
    /// let mut managers = Relation::new(rel_type);
    /// managers.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
    ///
    /// // Find employees who are also managers
    /// let managing_employees = current_employees.intersect(&managers).unwrap();
    /// assert_eq!(managing_employees.cardinality(), 1);  // Bob
    /// ```
    pub fn intersect(&self, other: &Relation) -> Result<Self, IntersectError> {
        // Check type compatibility
        if self.relation_type() != other.relation_type() {
            return Err(IntersectError);
        }

        // Optimization: If either relation is empty, the intersection is empty.
        // By checking this early, we avoid setting up iterators or cloning any tuples
        // for vacuous intersections.
        if self.is_empty() || other.is_empty() {
            return Ok(Relation::new(self.relation_type().clone()));
        }

        // Optimization: always iterate over the smaller relation and check membership
        // in the larger relation. This minimizes the number of O(1) hash lookups
        // and `.clone()` operations for the result.
        let (smaller, larger) = if self.cardinality() <= other.cardinality() {
            (self, other)
        } else {
            (other, self)
        };

        // Filter tuples and create relation without redundant checks
        // Safety: source tuples are from a valid relation of the same type
        Ok(Relation::from_tuples_unchecked(
            self.relation_type().clone(),
            smaller
                .tuples()
                .filter(|tuple| larger.contains(tuple))
                .cloned(),
        ))
    }

    /// Computes the intersection of this relation with another, consuming this relation.
    ///
    /// This is an optimized version of `intersect` that avoids O(N) tuple clones for the
    /// first relation by consuming it and mutates the inner body in place.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// let mut r1 = Relation::new(rel_type.clone());
    /// r1.insert(tuple! { id: 1i64 }).unwrap();
    /// let mut r2 = Relation::new(rel_type);
    /// r2.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// let result = r1.intersect_into(&r2).unwrap();
    /// assert_eq!(result.cardinality(), 1);
    /// ```
    pub fn intersect_into(mut self, other: &Relation) -> Result<Self, IntersectError> {
        // Check type compatibility
        if self.relation_type() != other.relation_type() {
            return Err(IntersectError);
        }

        self.body.retain(|tuple| other.contains(tuple));
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
    fn test_intersect_returns_common_tuples() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
        rel1.insert(tuple! { emp_id: 3i64, name: "Charlie" })
            .unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
        rel2.insert(tuple! { emp_id: 3i64, name: "Charlie" })
            .unwrap();
        rel2.insert(tuple! { emp_id: 4i64, name: "David" }).unwrap();

        let result = rel1.intersect(&rel2).unwrap();

        assert_eq!(result.cardinality(), 2); // Bob and Charlie

        let bob = tuple! { emp_id: 2i64, name: "Bob" };
        let charlie = tuple! { emp_id: 3i64, name: "Charlie" };

        assert!(result.contains(&bob));
        assert!(result.contains(&charlie));
    }

    #[test]
    fn test_intersect_type_mismatch() {
        let type1 = emp_type();
        let rel_type1 = RelationType::new(type1);
        let rel1 = Relation::new(rel_type1);

        let type2 = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("salary", ScalarType::Float);

        let rel_type2 = RelationType::new(type2);
        let rel2 = Relation::new(rel_type2);

        let result = rel1.intersect(&rel2);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), IntersectError));
    }

    #[test]
    fn test_intersect_with_empty_relation() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let rel2 = Relation::new(rel_type);

        let result = rel1.intersect(&rel2).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_intersect_no_common_tuples() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let result = rel1.intersect(&rel2).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_intersect_identical_relations() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type);
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let result = rel1.intersect(&rel1).unwrap();
        assert_eq!(result, rel1);
    }

    #[test]
    fn test_intersect_into_returns_common_tuples() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
        rel1.insert(tuple! { emp_id: 3i64, name: "Charlie" })
            .unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
        rel2.insert(tuple! { emp_id: 3i64, name: "Charlie" })
            .unwrap();
        rel2.insert(tuple! { emp_id: 4i64, name: "David" }).unwrap();

        let result = rel1.intersect_into(&rel2).unwrap();

        assert_eq!(result.cardinality(), 2); // Bob and Charlie

        let bob = tuple! { emp_id: 2i64, name: "Bob" };
        let charlie = tuple! { emp_id: 3i64, name: "Charlie" };

        assert!(result.contains(&bob));
        assert!(result.contains(&charlie));
    }

    #[test]
    fn test_intersect_into_type_mismatch() {
        let h1 = TupleType::new().with_attribute("x".to_string(), ScalarType::Int);
        let h2 = TupleType::new().with_attribute("y".to_string(), ScalarType::Int);

        let r1 = Relation::new(RelationType::new(h1));
        let r2 = Relation::new(RelationType::new(h2));

        let res = r1.intersect_into(&r2);
        assert!(res.is_err());
    }

    #[test]
    fn test_intersect_into_success() {
        let h = TupleType::new().with_attribute("x".to_string(), ScalarType::Int);
        let mut r1 = Relation::new(RelationType::new(h.clone()));
        r1.insert(crate::tuple! { x: 1i64 }).unwrap();
        r1.insert(crate::tuple! { x: 2i64 }).unwrap();

        let mut r2 = Relation::new(RelationType::new(h));
        r2.insert(crate::tuple! { x: 2i64 }).unwrap();
        r2.insert(crate::tuple! { x: 3i64 }).unwrap();

        let res = r1.intersect_into(&r2);
        assert!(res.is_ok());
        let res_rel = res.unwrap();
        assert_eq!(res_rel.tuples().count(), 1);
        assert_eq!(
            res_rel
                .tuples()
                .next()
                .unwrap()
                .get_typed::<i64>("x")
                .unwrap(),
            2i64
        );
    }
}
