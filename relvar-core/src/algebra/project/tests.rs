use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

#[test]
fn test_project_selects_attributes() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("dept_id", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
        .unwrap();

    let result = relation.project(&["emp_id", "name"]);

    assert_eq!(result.degree(), 2);
    assert_eq!(result.cardinality(), 2);

    // Verify result has correct attributes
    let alice_projected = tuple! { emp_id: 1i64, name: "Alice" };
    assert!(result.contains(&alice_projected));
}

#[test]
fn test_project_removes_duplicates() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("dept_id", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 10i64 })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 3i64, name: "Charlie", dept_id: 10i64 })
        .unwrap();

    // Project onto just dept_id - should have only one unique value
    let result = relation.project(&["dept_id"]);

    assert_eq!(result.degree(), 1);
    assert_eq!(result.cardinality(), 1); // All have same dept_id, duplicates removed
}

#[test]
fn test_project_empty_relation() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    let rel_type = RelationType::new(heading);
    let relation = Relation::new(rel_type);

    let result = relation.project(&["emp_id"]);

    assert_eq!(result.cardinality(), 0);
    assert!(result.is_empty());
}

#[test]
fn test_project_all_attributes() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice" })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, name: "Bob" })
        .unwrap();

    let result = relation.project(&["emp_id", "name"]);

    assert_eq!(result, relation);
}

#[test]
fn test_project_single_attribute() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("dept_id", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
        .unwrap();

    let result = relation.project(&["name"]);

    assert_eq!(result.degree(), 1);
    assert_eq!(result.cardinality(), 2);
}

/// Verifies TTM behavior for TABLE_DEE (empty heading, 1 tuple)
///
/// When projecting a non-empty relation onto an empty set of attributes,
/// the result should be a relation with degree 0 and cardinality 1.
/// This is equivalent to TABLE_DEE (Boolean TRUE).
#[test]
fn test_project_empty_attributes_creates_table_dee() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice" })
        .unwrap();
    relation
        .insert(tuple! { emp_id: 2i64, name: "Bob" })
        .unwrap();

    // Project onto empty set of attributes
    let result = relation.project(&[]);

    // Should have degree 0 (no attributes)
    assert_eq!(result.degree(), 0);

    // Should have cardinality 1 (one empty tuple)
    // Since {Alice} becomes {} and {Bob} becomes {}, they are duplicates
    // and reduced to a single {}.
    assert_eq!(result.cardinality(), 1);

    // Verify the single tuple is empty
    let tuple = result.tuples().next().unwrap();
    assert_eq!(tuple.degree(), 0);
}

/// Verifies TTM behavior for TABLE_DUM (empty heading, 0 tuples)
///
/// When projecting an empty relation onto an empty set of attributes,
/// the result should be a relation with degree 0 and cardinality 0.
/// This is equivalent to TABLE_DUM (Boolean FALSE).
#[test]
fn test_project_empty_attributes_on_empty_relation_creates_table_dum() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    let rel_type = RelationType::new(heading);
    let relation = Relation::new(rel_type);

    // Project onto empty set of attributes
    let result = relation.project(&[]);

    // Should have degree 0
    assert_eq!(result.degree(), 0);

    // Should have cardinality 0
    assert_eq!(result.cardinality(), 0);
    assert!(result.is_empty());
}
