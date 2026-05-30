use relvar_core::algebra::ExtendError;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

#[test]
fn test_extend_into_fails_if_attribute_exists() {
    let heading = TupleType::new()
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation
        .insert(tuple! { emp_id: 1i64, name: "Alice" })
        .unwrap();

    let result = relation.extend_into("name", ScalarType::String, |t| {
        t.get("name").unwrap().clone()
    });

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ExtendError::AttributeExists(_)
    ));
}

#[test]
fn test_extend_into_type_mismatch() {
    let heading = TupleType::new().with_attribute("emp_id".to_string(), ScalarType::Int);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    relation.insert(tuple! { emp_id: 1i64 }).unwrap();

    let result = relation.extend_into("name_length", ScalarType::String, |_| ScalarValue::Int(10));

    assert!(result.is_err());
    match result.unwrap_err() {
        ExtendError::TupleCreation(msg) => {
            assert!(msg.contains("Type mismatch"));
            assert!(msg.contains("expected String"));
            assert!(msg.contains("got Int"));
        }
        _ => panic!("Expected TupleCreation error"),
    }
}
