use relvar_core::algebra::delta::Delta;
use relvar_core::error::DatabaseError;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;

fn test_type_1() -> RelationType {
    RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int))
}

fn test_type_2() -> RelationType {
    RelationType::new(TupleType::new().with_attribute("y", ScalarType::Int))
}

#[test]
fn test_delta_new_type_mismatch() {
    let r1 = Relation::new(test_type_1());
    let r2 = Relation::new(test_type_2());

    let result = Delta::new(r1, r2);
    assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
}

#[test]
fn test_delta_between_type_mismatch() {
    let r1 = Relation::new(test_type_1());
    let r2 = Relation::new(test_type_2());

    let result = Delta::between(&r1, &r2);
    assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
}

#[test]
fn test_delta_apply_type_mismatch() {
    let r1 = Relation::new(test_type_1());
    let r2 = Relation::new(test_type_1());
    let r3 = Relation::new(test_type_2());

    let delta = Delta::between(&r1, &r2).unwrap();
    let result = delta.apply(&r3);
    assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
}

#[test]
fn test_delta_compose_type_mismatch() {
    let r1 = Relation::new(test_type_1());
    let r2 = Relation::new(test_type_1());
    let r3 = Relation::new(test_type_2());
    let r4 = Relation::new(test_type_2());

    let d1 = Delta::between(&r1, &r2).unwrap();
    let d2 = Delta::between(&r3, &r4).unwrap();

    let result = d1.compose(&d2);
    assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
}
