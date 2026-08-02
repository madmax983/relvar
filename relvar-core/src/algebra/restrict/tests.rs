use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue};

#[test]
fn test_restrict_filters_tuples() {
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
    relation
        .insert(tuple! { emp_id: 3i64, name: "Charlie", dept_id: 10i64 })
        .unwrap();

    // Filter by dept_id == 10
    let result = relation.restrict(|t| {
        if let Some(ScalarValue::Int(dept_id)) = t.get("dept_id") {
            *dept_id == 10
        } else {
            false
        }
    });

    assert_eq!(result.cardinality(), 2);

    let alice = tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 };
    let charlie = tuple! { emp_id: 3i64, name: "Charlie", dept_id: 10i64 };

    assert!(result.contains(&alice));
    assert!(result.contains(&charlie));
}

#[test]
fn test_restrict_on_empty_relation() {
    let heading = TupleType::new()
        .with_attribute("emp_id", ScalarType::Int)
        .with_attribute("name", ScalarType::String);

    let rel_type = RelationType::new(heading);
    let relation = Relation::new(rel_type);

    let result = relation.restrict(|_| true);

    assert_eq!(result.cardinality(), 0);
    assert!(result.is_empty());
}

#[test]
fn test_restrict_no_matches() {
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

    // Filter that matches nothing
    let result = relation.restrict(|_| false);

    assert_eq!(result.cardinality(), 0);
    assert!(result.is_empty());
}

#[test]
fn test_restrict_all_match() {
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

    // Filter that matches everything
    let result = relation.restrict(|_| true);

    assert_eq!(result.cardinality(), 2);
    assert_eq!(result, relation);
}
