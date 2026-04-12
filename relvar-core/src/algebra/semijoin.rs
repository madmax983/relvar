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

use crate::values::{Relation, Tuple};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

/// A key for hash join that avoids allocating a Vec for the key.
/// It holds references to the tuple and the attributes to key on.
#[derive(Debug, Eq)]
struct SemijoinKey<'t, 'a> {
    tuple: &'t Tuple,
    attributes: &'a [String],
}

impl<'t, 'a> PartialEq for SemijoinKey<'t, 'a> {
    fn eq(&self, other: &Self) -> bool {
        // We assume attributes are the same (or same values) as this is used internally
        // with the same common_attrs slice.
        for (i, attr) in self.attributes.iter().enumerate() {
            let v1 = self.tuple.get(attr);
            let v2 = other.tuple.get(&other.attributes[i]);
            if v1 != v2 {
                return false;
            }
        }
        true
    }
}

impl<'t, 'a> Hash for SemijoinKey<'t, 'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for attr in self.attributes {
            if let Some(val) = self.tuple.get(attr) {
                val.hash(state);
            }
        }
    }
}

/// Finds common attribute names between two relations' headings.
fn common_attributes(a: &Relation, b: &Relation) -> Vec<String> {
    a.relation_type()
        .heading()
        .attribute_names()
        .filter(|attr| b.relation_type().heading().has_attribute(attr))
        .cloned()
        .collect()
}

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
    /// O(n + m) where n and m are the cardinalities of the two relations.
    /// Optimized using a hash-based lookup.
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
        let common_attrs = common_attributes(self, other);

        // If no common attributes, we have a degenerate case (Cartesian product projection)
        if common_attrs.is_empty() {
            if !other.is_empty() {
                // If B is not empty, A MATCHING B = A (vacuous match)
                return self.clone();
            } else {
                // If B is empty, A MATCHING B = {}
                return Relation::new(self.relation_type().clone());
            }
        }

        // Build HashSet of keys from other relation
        // We use SemijoinKey to avoid allocating Vec per tuple
        let mut other_keys: HashSet<SemijoinKey> = HashSet::with_capacity(other.cardinality());

        for tuple in other.tuples() {
            other_keys.insert(SemijoinKey {
                tuple,
                attributes: &common_attrs,
            });
        }

        // Optimization: Pass an iterator directly to `from_tuples_unchecked` instead of
        // collecting into an intermediate `Vec`. The tuples are guaranteed to be valid
        // since they come from `self`.
        Relation::from_tuples_unchecked(
            self.relation_type().clone(),
            self.tuples()
                .filter(|tuple| {
                    let key = SemijoinKey {
                        tuple,
                        attributes: &common_attrs,
                    };
                    other_keys.contains(&key)
                })
                .cloned(),
        )
    }

    /// Alias for [`semijoin`](Self::semijoin) using Tutorial D naming (MATCHING).
    ///
    /// # Example
    ///
    /// ```text
    /// // Example
    /// ```
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
    /// O(n + m) where n and m are the cardinalities of the two relations.
    /// Optimized using a hash-based lookup.
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
        let common_attrs = common_attributes(self, other);

        // Degenerate case handling
        if common_attrs.is_empty() {
            if !other.is_empty() {
                // If B is not empty, A MATCHING B = A, so A MINUS A = {}
                return Relation::new(self.relation_type().clone());
            } else {
                // If B is empty, A MATCHING B = {}, so A MINUS {} = A
                return self.clone();
            }
        }

        // Build HashSet of keys from other relation
        let mut other_keys: HashSet<SemijoinKey> = HashSet::with_capacity(other.cardinality());

        for tuple in other.tuples() {
            other_keys.insert(SemijoinKey {
                tuple,
                attributes: &common_attrs,
            });
        }

        // Optimization: Pass an iterator directly to `from_tuples_unchecked` instead of
        // collecting into an intermediate `Vec`. The tuples are guaranteed to be valid
        // since they come from `self`.
        Relation::from_tuples_unchecked(
            self.relation_type().clone(),
            self.tuples()
                .filter(|tuple| {
                    let key = SemijoinKey {
                        tuple,
                        attributes: &common_attrs,
                    };
                    !other_keys.contains(&key)
                })
                .cloned(),
        )
    }

    /// Alias for [`semidifference`](Self::semidifference) using Tutorial D naming (NOT MATCHING).
    ///
    /// # Example
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn not_matching(&self, other: &Relation) -> Self {
        self.semidifference(other)
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
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 3i64, name: "Charlie", dept_id: 30i64 })
            .unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();
        departments
            .insert(tuple! { dept_id: 20i64, dept_name: "Sales" })
            .unwrap();
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
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();

        let result = employees.semijoin(&departments);
        assert!(result.is_empty());
    }

    #[test]
    fn test_semijoin_empty_other() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

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
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 99i64 })
            .unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();

        let result = employees.semijoin(&departments);
        assert!(result.is_empty());
    }

    #[test]
    fn test_semijoin_all_match() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();
        departments
            .insert(tuple! { dept_id: 20i64, dept_name: "Sales" })
            .unwrap();

        let result = employees.semijoin(&departments);
        assert_eq!(result, employees);
    }

    #[test]
    fn test_semijoin_preserves_heading() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();

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
        rel_a
            .insert(tuple! { x: 1i64, y: 10i64, data: "a" })
            .unwrap();
        rel_a
            .insert(tuple! { x: 1i64, y: 20i64, data: "b" })
            .unwrap();
        rel_a
            .insert(tuple! { x: 2i64, y: 10i64, data: "c" })
            .unwrap();

        let heading_b = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int)
            .with_attribute("label", ScalarType::String);
        let mut rel_b = Relation::new(RelationType::new(heading_b));
        rel_b
            .insert(tuple! { x: 1i64, y: 10i64, label: "match" })
            .unwrap();

        let result = rel_a.semijoin(&rel_b);
        // Only (x=1, y=10) matches both common attrs
        assert_eq!(result.cardinality(), 1);
        assert!(result.contains(&tuple! { x: 1i64, y: 10i64, data: "a" }));
    }

    #[test]
    fn test_matching_alias() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();

        assert_eq!(
            employees.matching(&departments),
            employees.semijoin(&departments)
        );
    }

    #[test]
    fn test_semidifference_returns_non_matching_tuples() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 3i64, name: "Charlie", dept_id: 30i64 })
            .unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();
        departments
            .insert(tuple! { dept_id: 20i64, dept_name: "Sales" })
            .unwrap();

        let result = employees.semidifference(&departments);

        // Only Charlie (dept_id 30 has no match)
        assert_eq!(result.cardinality(), 1);
        assert_eq!(result.degree(), 3);
        assert!(result.contains(&tuple! { emp_id: 3i64, name: "Charlie", dept_id: 30i64 }));
        assert!(!result.contains(&tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }));
    }

    #[test]
    fn test_semidifference_empty_self() {
        let employees = Relation::new(RelationType::new(emp_heading()));
        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();

        let result = employees.semidifference(&departments);
        assert!(result.is_empty());
    }

    #[test]
    fn test_semidifference_empty_other() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let departments = Relation::new(RelationType::new(dept_heading()));

        let result = employees.semidifference(&departments);
        assert_eq!(result, employees);
    }

    #[test]
    fn test_semidifference_both_empty() {
        let employees = Relation::new(RelationType::new(emp_heading()));
        let departments = Relation::new(RelationType::new(dept_heading()));

        let result = employees.semidifference(&departments);
        assert!(result.is_empty());
    }

    #[test]
    fn test_semidifference_no_common_attributes_other_non_empty() {
        // Disjoint headings + B non-empty => result = A (vacuous match)
        let heading_a = TupleType::new().with_attribute("a", ScalarType::Int);
        let mut rel_a = Relation::new(RelationType::new(heading_a));
        rel_a.insert(tuple! { a: 1i64 }).unwrap();

        let heading_b = TupleType::new().with_attribute("b", ScalarType::String);
        let mut rel_b = Relation::new(RelationType::new(heading_b));
        rel_b.insert(tuple! { b: "x" }).unwrap();

        let result = rel_a.semidifference(&rel_b);
        assert!(result.is_empty());
    }

    #[test]
    fn test_semidifference_no_common_attributes_other_empty() {
        // Disjoint headings + B empty => result = empty
        let heading_a = TupleType::new().with_attribute("a", ScalarType::Int);
        let mut rel_a = Relation::new(RelationType::new(heading_a));
        rel_a.insert(tuple! { a: 1i64 }).unwrap();

        let heading_b = TupleType::new().with_attribute("b", ScalarType::String);
        let rel_b = Relation::new(RelationType::new(heading_b));

        let result = rel_a.semidifference(&rel_b);
        assert_eq!(result, rel_a);
    }

    #[test]
    fn test_semidifference_same_heading_equals_difference() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);

        let mut rel_a = Relation::new(rel_type.clone());
        rel_a.insert(tuple! { id: 1i64 }).unwrap();
        rel_a.insert(tuple! { id: 2i64 }).unwrap();
        rel_a.insert(tuple! { id: 3i64 }).unwrap();

        let mut rel_b = Relation::new(rel_type);
        rel_b.insert(tuple! { id: 2i64 }).unwrap();
        rel_b.insert(tuple! { id: 3i64 }).unwrap();

        let semidiff_result = rel_a.semidifference(&rel_b);
        let diff_result = rel_a.difference(&rel_b).unwrap();

        assert_eq!(semidiff_result, diff_result);
    }

    #[test]
    fn test_semidifference_no_matches() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 99i64 })
            .unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();

        let result = employees.semidifference(&departments);
        assert_eq!(result, employees);
    }

    #[test]
    fn test_semidifference_all_match() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();

        let result = employees.semidifference(&departments);
        assert!(result.is_empty());
    }

    #[test]
    fn test_semidifference_preserves_heading() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let departments = Relation::new(RelationType::new(dept_heading()));

        let result = employees.semidifference(&departments);
        assert_eq!(result.relation_type(), employees.relation_type());
    }

    #[test]
    fn test_semidifference_multiple_common_attributes() {
        let heading_a = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int)
            .with_attribute("data", ScalarType::String);
        let mut rel_a = Relation::new(RelationType::new(heading_a));
        rel_a
            .insert(tuple! { x: 1i64, y: 10i64, data: "a" })
            .unwrap();
        rel_a
            .insert(tuple! { x: 1i64, y: 20i64, data: "b" })
            .unwrap();
        rel_a
            .insert(tuple! { x: 2i64, y: 10i64, data: "c" })
            .unwrap();

        let heading_b = TupleType::new()
            .with_attribute("x", ScalarType::Int)
            .with_attribute("y", ScalarType::Int)
            .with_attribute("label", ScalarType::String);
        let mut rel_b = Relation::new(RelationType::new(heading_b));
        rel_b
            .insert(tuple! { x: 1i64, y: 10i64, label: "match" })
            .unwrap();

        let result = rel_a.semidifference(&rel_b);
        assert_eq!(result.cardinality(), 2);
        assert!(result.contains(&tuple! { x: 1i64, y: 20i64, data: "b" }));
        assert!(result.contains(&tuple! { x: 2i64, y: 10i64, data: "c" }));
    }

    #[test]
    fn test_not_matching_alias() {
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let departments = Relation::new(RelationType::new(dept_heading()));

        assert_eq!(
            employees.not_matching(&departments),
            employees.semidifference(&departments)
        );
    }

    #[test]
    fn test_semijoin_union_semidifference_equals_self() {
        // Partition law: A = (A SEMIJOIN B) UNION (A SEMIDIFFERENCE B)
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 3i64, name: "Charlie", dept_id: 30i64 })
            .unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();

        let matched = employees.semijoin(&departments);
        let unmatched = employees.semidifference(&departments);
        let reunited = matched.union(&unmatched).unwrap();

        assert_eq!(reunited, employees);
    }

    #[test]
    fn test_semijoin_equals_join_project() {
        // Formal definition: A SEMIJOIN B = (A JOIN B) PROJECT {attrs of A}
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 3i64, name: "Charlie", dept_id: 30i64 })
            .unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();
        departments
            .insert(tuple! { dept_id: 20i64, dept_name: "Sales" })
            .unwrap();

        let direct = employees.semijoin(&departments);
        let via_join = employees
            .join(&departments)
            .unwrap()
            .project(&["emp_id", "name", "dept_id"]);

        assert_eq!(direct, via_join);
    }

    #[test]
    fn test_semidifference_equals_self_minus_semijoin() {
        // Formal definition: A SEMIDIFFERENCE B = A MINUS (A SEMIJOIN B)
        let mut employees = Relation::new(RelationType::new(emp_heading()));
        employees
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();
        employees
            .insert(tuple! { emp_id: 3i64, name: "Charlie", dept_id: 30i64 })
            .unwrap();

        let mut departments = Relation::new(RelationType::new(dept_heading()));
        departments
            .insert(tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();

        let direct = employees.semidifference(&departments);
        let via_diff = employees
            .difference(&employees.semijoin(&departments))
            .unwrap();

        assert_eq!(direct, via_diff);
    }

    #[test]
    fn test_semijoin_key_missing_attributes() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hash;

        let t1 = tuple! { a: 1i64 };
        let attributes = vec!["a".to_string(), "b".to_string()];

        let key1 = super::SemijoinKey {
            tuple: &t1,
            attributes: &attributes,
        };

        // Should not panic on missing attribute "b", and hashing should complete
        let mut hasher = DefaultHasher::new();
        key1.hash(&mut hasher);

        let t2 = tuple! { a: 1i64, c: "test" };
        let key2 = super::SemijoinKey {
            tuple: &t2,
            attributes: &attributes,
        };

        // Ensure missing attributes don't cause panics during partial eq comparison
        assert_eq!(key1, key2);

        let t3 = tuple! { a: 1i64, b: 2i64 };
        let key3 = super::SemijoinKey {
            tuple: &t3,
            attributes: &attributes,
        };

        assert_ne!(key1, key3);
    }
}
