//! Semijoin and semidifference operators.
//!
//! These operators filter one relation based on the existence (or absence) of
//! matching tuples in another relation, matching on common attributes.
//!
//! - **Semijoin** (MATCHING) - Tuples from A that match some tuple in B
//! - **Semidifference** (NOT MATCHING / antijoin) - Tuples from A that match no tuple in B
//!
//! # TTM Compliance
//!
//! - RM Prescription 7: relational algebra completeness
//! - Formally: `A SEMIJOIN B = (A JOIN B) PROJECT {attributes of A}`
//! - Formally: `A SEMIDIFFERENCE B = A MINUS (A SEMIJOIN B)`
//! - Result heading always equals self's heading
//! - Set semantics maintained (no duplicates, no ordering)

use crate::values::Relation;

impl Relation {
    /// Computes the semijoin of this relation with another (A MATCHING B).
    ///
    /// Returns tuples from this relation that have at least one matching
    /// tuple in `other` on their common attributes. The result heading
    /// is always this relation's heading.
    ///
    /// Formally: `A SEMIJOIN B = (A JOIN B) PROJECT {attributes of A}`
    ///
    /// # Arguments
    ///
    /// * `other` - The relation to match against
    ///
    /// # Returns
    ///
    /// A new relation with this relation's heading, containing only the
    /// tuples that have a match in `other`.
    ///
    /// # Behavior
    ///
    /// - If there are no common attributes and `other` is non-empty, returns `self`
    ///   (vacuous match, like Cartesian product projected back)
    /// - If `other` is empty, returns an empty relation
    /// - If headings are identical, equivalent to [`intersect()`](Self::intersect)
    ///
    /// # Complexity
    ///
    /// O(n * m) where n and m are the cardinalities of the two relations.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let emp_heading = TupleType::new()
    ///     .with_attribute("emp_id", ScalarType::Int)
    ///     .with_attribute("dept_id", ScalarType::Int);
    /// let mut employees = Relation::new(RelationType::new(emp_heading));
    /// employees.insert(tuple! { emp_id: 1i64, dept_id: 10i64 }).unwrap();
    /// employees.insert(tuple! { emp_id: 2i64, dept_id: 20i64 }).unwrap();
    ///
    /// let dept_heading = TupleType::new()
    ///     .with_attribute("dept_id", ScalarType::Int)
    ///     .with_attribute("dept_name", ScalarType::String);
    /// let mut departments = Relation::new(RelationType::new(dept_heading));
    /// departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();
    ///
    /// let result = employees.semijoin(&departments);
    /// assert_eq!(result.cardinality(), 1); // Only emp 1 matches
    /// ```
    pub fn semijoin(&self, other: &Relation) -> Self {
        let common_attrs: Vec<String> = self
            .relation_type()
            .heading()
            .attribute_names()
            .filter(|attr| other.relation_type().heading().has_attribute(attr))
            .cloned()
            .collect();

        let matched: Vec<_> = self
            .tuples()
            .filter(|tuple| {
                other.tuples().any(|other_tuple| {
                    common_attrs
                        .iter()
                        .all(|attr| tuple.get(attr) == other_tuple.get(attr))
                })
            })
            .cloned()
            .collect();

        Relation::from_tuples(self.relation_type().clone(), matched)
            .expect("Semijoin tuples conform to self's relation type")
    }
}

#[cfg(test)]
mod tests {
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    fn emp_heading() -> TupleType {
        TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int)
    }

    fn dept_heading() -> TupleType {
        TupleType::new()
            .with_attribute("dept_id", ScalarType::Int)
            .with_attribute("dept_name", ScalarType::String)
    }

    #[test]
    fn test_semijoin_returns_matching_tuples() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();
        employees.insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 }).unwrap();
        employees.insert(tuple! { emp_id: 3i64, name: "Charlie", dept_id: 30i64 }).unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();
        departments.insert(tuple! { dept_id: 20i64, dept_name: "Sales" }).unwrap();
        // dept_id 30 is absent

        let result = employees.semijoin(&departments);

        // Only Alice and Bob have matching dept_ids
        assert_eq!(result.cardinality(), 2);
        assert_eq!(result.degree(), 3); // emp_id, name, dept_id (self's heading)
        assert!(result.contains(&tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }));
        assert!(result.contains(&tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 }));
        assert!(!result.contains(&tuple! { emp_id: 3i64, name: "Charlie", dept_id: 30i64 }));
    }
}
