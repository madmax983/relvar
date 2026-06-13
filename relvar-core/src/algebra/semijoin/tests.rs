// use super::*;
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

#[test]
fn test_semijoin_into_filters_in_place() {
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

    let result = employees.semijoin_into(&departments);

    assert_eq!(result.cardinality(), 1);
    assert!(result.contains(&tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }));
}

#[test]
fn test_semidifference_into_filters_in_place() {
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

    let result = employees.semidifference_into(&departments);

    assert_eq!(result.cardinality(), 1);
    assert!(result.contains(&tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 }));
}

#[test]
fn test_not_matching_into_alias() {
    let mut employees = Relation::new(RelationType::new(emp_heading()));
    employees
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();

    let departments = Relation::new(RelationType::new(dept_heading()));

    let result = employees.not_matching_into(&departments);

    // Alias should behave identically
    assert_eq!(result.cardinality(), 1);
}
