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
    assert!(matches!(result.unwrap_err(), DifferenceError));
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

#[test]
fn test_difference_into_returns_tuples_in_first_but_not_second() {
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

    let result = rel1.difference_into(&rel2).unwrap();

    assert_eq!(result.cardinality(), 2); // Alice and Charlie

    let alice = tuple! { emp_id: 1i64, name: "Alice" };
    let charlie = tuple! { emp_id: 3i64, name: "Charlie" };

    assert!(result.contains(&alice));
    assert!(result.contains(&charlie));
    assert!(!result.contains(&tuple! { emp_id: 2i64, name: "Bob" }));
}

#[test]
fn test_minus_into_alias() {
    let heading = emp_type();
    let rel_type = RelationType::new(heading);

    let mut rel1 = Relation::new(rel_type.clone());
    rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

    let mut rel2 = Relation::new(rel_type);
    rel2.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

    let result = rel1.minus_into(&rel2).unwrap();
    assert!(result.is_empty());
}
