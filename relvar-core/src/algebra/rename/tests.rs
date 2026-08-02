use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

#[test]
fn test_rename_into() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("dept_id", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();

    let renamed = relation.rename_into(&[("emp_id", "employee_id"), ("dept_id", "department_id")]);

    assert!(
        renamed
            .relation_type()
            .heading()
            .has_attribute("employee_id")
    );
    assert!(
        renamed
            .relation_type()
            .heading()
            .has_attribute("department_id")
    );
    assert!(renamed.relation_type().heading().has_attribute("name"));
    assert!(!renamed.relation_type().heading().has_attribute("emp_id"));
    assert!(!renamed.relation_type().heading().has_attribute("dept_id"));
    assert_eq!(renamed.cardinality(), 1);

    let tuple = renamed.tuples().next().unwrap();
    assert_eq!(tuple.get_typed::<i64>("employee_id").unwrap(), 1);
    assert_eq!(
        tuple.get_typed::<String>("name").unwrap(),
        "Alice".to_string()
    );
    assert_eq!(tuple.get_typed::<i64>("department_id").unwrap(), 10);
}

#[test]
fn test_rename_changes_attribute_name() {
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

    let result = relation.rename(&[("emp_id", "id")]);

    // Check that new attribute exists and old doesn't
    assert!(result.relation_type().heading().has_attribute("id"));
    assert!(!result.relation_type().heading().has_attribute("emp_id"));
    assert!(result.relation_type().heading().has_attribute("name"));

    // Check cardinality preserved
    assert_eq!(result.cardinality(), 2);
}

#[test]
fn test_rename_preserves_type() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice" })
        .unwrap();

    let result = relation.rename(&[("emp_id", "employee_id")]);

    // Verify type preserved
    assert_eq!(
        result
            .relation_type()
            .heading()
            .get_attribute_type("employee_id"),
        Some(&ScalarType::Int)
    );
}

#[test]
fn test_rename_multiple_attributes() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("dept_id", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
        .unwrap();

    let result = relation.rename(&[("emp_id", "employee_id"), ("dept_id", "department_id")]);

    assert!(
        result
            .relation_type()
            .heading()
            .has_attribute("employee_id")
    );
    assert!(
        result
            .relation_type()
            .heading()
            .has_attribute("department_id")
    );
    assert!(result.relation_type().heading().has_attribute("name"));
    assert!(!result.relation_type().heading().has_attribute("emp_id"));
    assert!(!result.relation_type().heading().has_attribute("dept_id"));
}

#[test]
fn test_rename_empty_mappings() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    let rel_type = RelationType::new(heading.clone());
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice" })
        .unwrap();

    let result = relation.rename(&[]);

    // Should be unchanged
    assert_eq!(result.relation_type().heading(), &heading);
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_rename_on_empty_relation() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    let rel_type = RelationType::new(heading);
    let relation = Relation::new(rel_type);

    let result = relation.rename(&[("emp_id", "id")]);

    assert!(result.is_empty());
    assert!(result.relation_type().heading().has_attribute("id"));
}

/// Verifies the behavior when multiple attributes are renamed to the same name.
///
/// CURRENT BEHAVIOR: Silent overwriting.
/// Since iteration over attributes is based on BTreeMap (sorted by name),
/// the attribute that comes later lexicographically overwrites earlier ones.
///
/// This test documents this "Last Write Wins" behavior.
#[test]
fn test_rename_collision_overwrites_values() {
    let heading = TupleType::new()
        .with_attribute("A", ScalarType::Int)
        .with_attribute("B", ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation.insert(tuple! { A: 1i64, B: 2i64 }).unwrap();

    // Rename both A and B to C
    // A comes before B, so we expect B to overwrite A.
    let result = relation.rename(&[("A", "C"), ("B", "C")]);

    assert_eq!(result.degree(), 1);
    assert!(result.relation_type().heading().has_attribute("C"));

    let tuple = result.tuples().next().unwrap();
    // Value should be 2 (from B)
    assert_eq!(tuple.get_typed::<i64>("C").unwrap(), 2);
}
