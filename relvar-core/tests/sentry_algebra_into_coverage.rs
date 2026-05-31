use relvar_core::DatabaseError;
use relvar_core::algebra::{DifferenceError, ExtendError, IntersectError, UnionError};
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

#[test]
fn test_intersect_into_type_mismatch() {
    let heading1 = TupleType::new().with_attribute("A".to_string(), ScalarType::Int);
    let heading2 = TupleType::new().with_attribute("B".to_string(), ScalarType::Int);
    let r1 = Relation::new(RelationType::new(heading1));
    let r2 = Relation::new(RelationType::new(heading2));

    assert!(matches!(r1.intersect_into(&r2), Err(IntersectError)));
}

#[test]
fn test_union_into_type_mismatch() {
    let heading1 = TupleType::new().with_attribute("A".to_string(), ScalarType::Int);
    let heading2 = TupleType::new().with_attribute("B".to_string(), ScalarType::Int);
    let r1 = Relation::new(RelationType::new(heading1));
    let r2 = Relation::new(RelationType::new(heading2));

    assert!(matches!(r1.union_into(&r2), Err(UnionError)));
}

#[test]
fn test_difference_into_type_mismatch() {
    let heading1 = TupleType::new().with_attribute("A".to_string(), ScalarType::Int);
    let heading2 = TupleType::new().with_attribute("B".to_string(), ScalarType::Int);
    let r1 = Relation::new(RelationType::new(heading1));
    let r2 = Relation::new(RelationType::new(heading2));

    assert!(matches!(r1.difference_into(&r2), Err(DifferenceError)));

    let r1_b = Relation::new(RelationType::new(
        TupleType::new().with_attribute("A".to_string(), ScalarType::Int),
    ));
    assert!(matches!(r1_b.minus_into(&r2), Err(DifferenceError)));
}

#[test]
fn test_delta_compose_diff_err() {
    use relvar_core::algebra::Delta;
    let heading = TupleType::new().with_attribute("A".to_string(), ScalarType::Int);
    let mut r1 = Relation::new(RelationType::new(heading.clone()));
    r1.insert(tuple! { A: 1i64 }).unwrap();
    let mut r2 = Relation::new(RelationType::new(heading.clone()));
    r2.insert(tuple! { A: 2i64 }).unwrap();
    let mut r3 = Relation::new(RelationType::new(heading.clone()));
    r3.insert(tuple! { A: 3i64 }).unwrap();

    let mut d1 = Delta::between(&r1, &r2).unwrap();
    let d2 = Delta::between(&r2, &r3).unwrap();

    // forcefully corrupt d1's deleted to have different heading to trigger inner difference err
    let wrong_heading = TupleType::new().with_attribute("B".to_string(), ScalarType::Int);
    d1.deleted = Relation::new(RelationType::new(wrong_heading));

    assert!(matches!(
        d1.compose(&d2),
        Err(DatabaseError::AlgebraError(_))
    ));
}

#[test]
fn test_extend_into_empty() {
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let mut r = Relation::new(RelationType::new(heading));
    r.insert(tuple! { id: 1i64 }).unwrap();

    let result = r.extend_into("new_attr", ScalarType::Int, |_t| {
        ScalarValue::String("not an int".to_string())
    });
    assert!(matches!(result, Err(ExtendError::TupleCreation(_))));
}
