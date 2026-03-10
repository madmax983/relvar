//! Difference operator for set subtraction.
//!
//! The difference operator (also called "minus" or "except") returns tuples
//! that are in the first relation but not in the second relation.
//!
//! # TTM Compliance
//!
//! - Relations must be type-compatible (identical headings)
//! - Result contains tuples from first relation not in second
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
//! let mut all_employees = Relation::new(rel_type.clone());
//! all_employees.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
//! all_employees.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
//!
//! let mut terminated = Relation::new(rel_type);
//! terminated.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
//!
//! let active = all_employees.difference(&terminated).unwrap();
//! assert_eq!(active.cardinality(), 1);  // Only Alice remains
//! ```

use crate::error::DatabaseError;
use crate::values::Relation;

impl Relation {
    /// Computes the set difference of this relation with another.
    ///
    /// This is the set difference operator from relational algebra (A - B or
    /// A MINUS B in SQL). It produces a new relation containing only the tuples
    /// that appear in this relation but not in the other relation.
    ///
    /// # Arguments
    ///
    /// * `other` - The relation to subtract. Must have the same heading
    ///   (type) as this relation.
    ///
    /// # Returns
    ///
    /// A new relation containing tuples that are in `self` but not in `other`.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::AlgebraError`] if the relations have different
    /// headings (different attribute names or types).
    ///
    /// # Behavior
    ///
    /// - Both relations must be type-compatible (identical headings)
    /// - Tuples present in both relations are excluded from the result
    /// - Order matters: A - B is different from B - A
    /// - Difference with self returns empty relation
    /// - Difference with empty relation returns the original relation
    ///
    /// # Complexity
    ///
    /// O(n * m) where n and m are the cardinalities of the two relations,
    /// due to tuple membership testing.
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
    /// let mut all = Relation::new(rel_type.clone());
    /// all.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
    /// all.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
    /// all.insert(tuple! { emp_id: 3i64, name: "Charlie" }).unwrap();
    ///
    /// let mut subset = Relation::new(rel_type);
    /// subset.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
    ///
    /// let result = all.difference(&subset).unwrap();
    /// assert_eq!(result.cardinality(), 2);  // Alice and Charlie
    /// ```
    pub fn difference(&self, other: &Relation) -> Result<Self, DatabaseError> {
        // Check type compatibility
        if self.relation_type() != other.relation_type() {
            return Err(DatabaseError::AlgebraError(
                "Relations must have the same type (heading) for difference".to_string(),
            ));
        }

        // Filter tuples and create relation without redundant checks
        // Safety: source tuples are from a valid relation of the same type
        Ok(Relation::from_tuples_unchecked(
            self.relation_type().clone(),
            self.tuples()
                .filter(|tuple| !other.contains(tuple))
                .cloned(),
        ))
    }

    /// Alias for [`difference`](Self::difference) with SQL-style naming.
    ///
    /// This method is identical to `difference()` but uses the SQL-style
    /// name "minus" which some users may find more intuitive.
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
    /// let rel_type = RelationType::new(heading);
    ///
    /// let mut rel1 = Relation::new(rel_type.clone());
    /// rel1.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// let rel2 = Relation::new(rel_type);
    ///
    /// let result = rel1.minus(&rel2).unwrap();
    /// assert_eq!(result.cardinality(), 1);
    /// ```
    pub fn minus(&self, other: &Relation) -> Result<Self, DatabaseError> {
        self.difference(other)
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
    fn test_difference_returns_tuples_in_first_but_not_second() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
        rel1.insert(tuple! { emp_id: 3i64, name: "Charlie" })
            .unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
        rel2.insert(tuple! { emp_id: 4i64, name: "David" }).unwrap();

        let result = rel1.difference(&rel2).unwrap();

        assert_eq!(result.cardinality(), 2); // Alice and Charlie

        let alice = tuple! { emp_id: 1i64, name: "Alice" };
        let charlie = tuple! { emp_id: 3i64, name: "Charlie" };

        assert!(result.contains(&alice));
        assert!(result.contains(&charlie));
        assert!(!result.contains(&tuple! { emp_id: 2i64, name: "Bob" }));
    }

    #[test]
    fn test_difference_type_mismatch() {
        let type1 = emp_type();
        let rel_type1 = RelationType::new(type1);
        let rel1 = Relation::new(rel_type1);

        let type2 = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("salary", ScalarType::Float);

        let rel_type2 = RelationType::new(type2);
        let rel2 = Relation::new(rel_type2);

        let result = rel1.difference(&rel2);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DatabaseError::AlgebraError(_)));
    }

    #[test]
    fn test_difference_with_empty_relation() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let rel2 = Relation::new(rel_type);

        let result = rel1.difference(&rel2).unwrap();
        assert_eq!(result, rel1); // All tuples remain
    }

    #[test]
    fn test_difference_all_removed() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let result = rel1.difference(&rel1).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_difference_no_common_tuples() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let result = rel1.difference(&rel2).unwrap();
        assert_eq!(result, rel1); // Nothing removed
    }

    #[test]
    fn test_minus_alias() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let result = rel1.minus(&rel2).unwrap();
        assert!(result.is_empty());
    }
}
