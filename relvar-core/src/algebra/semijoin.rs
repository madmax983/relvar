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

    /// Alias for [`semijoin`](Self::semijoin) using Tutorial D naming (MATCHING).
    pub fn matching(&self, other: &Relation) -> Self {
        self.semijoin(other)
    }

    /// Computes the semidifference of this relation with another (A NOT MATCHING B).
    ///
    /// Returns tuples from this relation that have NO matching tuple in
    /// `other` on their common attributes. The result heading is always
    /// this relation's heading. Also known as antijoin.
    ///
    /// Formally: `A SEMIDIFFERENCE B = A MINUS (A SEMIJOIN B)`
    ///
    /// # Arguments
    ///
    /// * `other` - The relation to match against
    ///
    /// # Returns
    ///
    /// A new relation with this relation's heading, containing only the
    /// tuples that have NO match in `other`.
    ///
    /// # Behavior
    ///
    /// - If there are no common attributes and `other` is non-empty, returns
    ///   an empty relation (all tuples vacuously match, so none remain)
    /// - If `other` is empty, returns `self` (no tuples to exclude)
    /// - If headings are identical, equivalent to [`difference()`](Self::difference)
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
    /// let result = employees.semidifference(&departments);
    /// assert_eq!(result.cardinality(), 1); // Only emp 2 (no matching dept)
    /// ```
    pub fn semidifference(&self, other: &Relation) -> Self {
        let common_attrs: Vec<String> = self
            .relation_type()
            .heading()
            .attribute_names()
            .filter(|attr| other.relation_type().heading().has_attribute(attr))
            .cloned()
            .collect();

        let non_matched: Vec<_> = self
            .tuples()
            .filter(|tuple| {
                !other.tuples().any(|other_tuple| {
                    common_attrs
                        .iter()
                        .all(|attr| tuple.get(attr) == other_tuple.get(attr))
                })
            })
            .cloned()
            .collect();

        Relation::from_tuples(self.relation_type().clone(), non_matched)
            .expect("Semidifference tuples conform to self's relation type")
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

    #[test]
    fn test_semijoin_empty_self() {
        let employees = Relation::new(RelationType::new(emp_heading()));
        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();

        let result = employees.semijoin(&departments);
        assert!(result.is_empty());
    }

    #[test]
    fn test_semijoin_empty_other() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();

        let departments = Relation::new(RelationType::new(dept_heading()));

        let result = employees.semijoin(&departments);
        assert!(result.is_empty());
    }

    #[test]
    fn test_semijoin_both_empty() {
        let employees = Relation::new(RelationType::new(emp_heading()));
        let departments = Relation::new(RelationType::new(dept_heading()));

        let result = employees.semijoin(&departments);
        assert!(result.is_empty());
    }

    #[test]
    fn test_semijoin_no_common_attributes_other_non_empty() {
        // Disjoint headings + B non-empty => result = A (vacuous match)
        let heading_a = TupleType::new().with_attribute("a", ScalarType::Int);
        let mut rel_a = Relation::new(RelationType::new(heading_a));
        rel_a.insert(tuple! { a: 1i64 }).unwrap();
        rel_a.insert(tuple! { a: 2i64 }).unwrap();

        let heading_b = TupleType::new().with_attribute("b", ScalarType::String);
        let mut rel_b = Relation::new(RelationType::new(heading_b));
        rel_b.insert(tuple! { b: "x" }).unwrap();

        let result = rel_a.semijoin(&rel_b);
        assert_eq!(result.cardinality(), 2);
        assert_eq!(result, rel_a);
    }

    #[test]
    fn test_semijoin_no_common_attributes_other_empty() {
        // Disjoint headings + B empty => result = empty
        let heading_a = TupleType::new().with_attribute("a", ScalarType::Int);
        let mut rel_a = Relation::new(RelationType::new(heading_a));
        rel_a.insert(tuple! { a: 1i64 }).unwrap();

        let heading_b = TupleType::new().with_attribute("b", ScalarType::String);
        let rel_b = Relation::new(RelationType::new(heading_b));

        let result = rel_a.semijoin(&rel_b);
        assert!(result.is_empty());
    }

    #[test]
    fn test_semijoin_same_heading_equals_intersect() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);

        let mut rel_a = Relation::new(rel_type.clone());
        rel_a.insert(tuple! { id: 1i64 }).unwrap();
        rel_a.insert(tuple! { id: 2i64 }).unwrap();
        rel_a.insert(tuple! { id: 3i64 }).unwrap();

        let mut rel_b = Relation::new(rel_type);
        rel_b.insert(tuple! { id: 2i64 }).unwrap();
        rel_b.insert(tuple! { id: 3i64 }).unwrap();
        rel_b.insert(tuple! { id: 4i64 }).unwrap();

        let semijoin_result = rel_a.semijoin(&rel_b);
        let intersect_result = rel_a.intersect(&rel_b).unwrap();

        assert_eq!(semijoin_result, intersect_result);
    }

    #[test]
    fn test_semijoin_no_matches() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 99i64 }).unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();

        let result = employees.semijoin(&departments);
        assert!(result.is_empty());
    }

    #[test]
    fn test_semijoin_all_match() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();
        employees.insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 }).unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();
        departments.insert(tuple! { dept_id: 20i64, dept_name: "Sales" }).unwrap();

        let result = employees.semijoin(&departments);
        assert_eq!(result, employees);
    }

    #[test]
    fn test_semijoin_preserves_heading() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();

        let result = employees.semijoin(&departments);
        assert_eq!(result.relation_type(), employees.relation_type());
    }

    #[test]
    fn test_semijoin_multiple_common_attributes() {
        let heading_a = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int)
            .with_attribute("data", ScalarType::String);
        let mut rel_a = Relation::new(RelationType::new(heading_a));
        rel_a.insert(tuple! { x: 1i64, y: 10i64, data: "a" }).unwrap();
        rel_a.insert(tuple! { x: 1i64, y: 20i64, data: "b" }).unwrap();
        rel_a.insert(tuple! { x: 2i64, y: 10i64, data: "c" }).unwrap();

        let heading_b = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int)
            .with_attribute("label", ScalarType::String);
        let mut rel_b = Relation::new(RelationType::new(heading_b));
        rel_b.insert(tuple! { x: 1i64, y: 10i64, label: "match" }).unwrap();

        let result = rel_a.semijoin(&rel_b);
        // Only (x=1, y=10) matches both common attrs
        assert_eq!(result.cardinality(), 1);
        assert!(result.contains(&tuple! { x: 1i64, y: 10i64, data: "a" }));
    }

    #[test]
    fn test_matching_alias() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();

        assert_eq!(
            employees.matching(&departments),
            employees.semijoin(&departments)
        );
    }

    #[test]
    fn test_semidifference_returns_non_matching_tuples() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();
        employees.insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 }).unwrap();
        employees.insert(tuple! { emp_id: 3i64, name: "Charlie", dept_id: 30i64 }).unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments.insert(tuple! { dept_id: 10i64, dept_name: "Engineering" }).unwrap();
        departments.insert(tuple! { dept_id: 20i64, dept_name: "Sales" }).unwrap();

        let result = employees.semidifference(&departments);

        // Only Charlie (dept_id 30 has no match)
        assert_eq!(result.cardinality(), 1);
        assert_eq!(result.degree(), 3);
        assert!(result.contains(&tuple! { emp_id: 3i64, name: "Charlie", dept_id: 30i64 }));
        assert!(!result.contains(&tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }));
    }
}
