use super::*;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};

fn test_type() -> RelationType {
    RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int))
}

#[test]
fn test_delta_between_empty() {
    let t = test_type();
    let r1 = Relation::new(t.clone());
    let r2 = Relation::new(t.clone());

    let delta = Delta::between(&r1, &r2).unwrap();
    assert!(delta.inserted.is_empty());
    assert!(delta.deleted.is_empty());
}

#[test]
fn test_delta_simple() {
    let t = test_type();
    let mut r1 = Relation::new(t.clone());
    r1.insert(tuple! { x: 1i64 }).unwrap();

    let mut r2 = Relation::new(t.clone());
    r2.insert(tuple! { x: 2i64 }).unwrap();

    // r1 -> r2: delete 1, insert 2
    let delta = Delta::between(&r1, &r2).unwrap();
    assert_eq!(delta.deleted.cardinality(), 1);
    assert!(delta.deleted.contains(&tuple! { x: 1i64 }));
    assert_eq!(delta.inserted.cardinality(), 1);
    assert!(delta.inserted.contains(&tuple! { x: 2i64 }));

    // Apply
    let r3 = delta.apply(&r1).unwrap();
    assert_eq!(r3, r2);
}

#[test]
fn test_invert() {
    let t = test_type();
    let mut r1 = Relation::new(t.clone());
    r1.insert(tuple! { x: 1i64 }).unwrap();

    let mut r2 = Relation::new(t.clone());
    r2.insert(tuple! { x: 2i64 }).unwrap();

    let delta = Delta::between(&r1, &r2).unwrap();
    let inv = delta.invert();

    // inv should be r2 -> r1: delete 2, insert 1
    assert!(inv.deleted.contains(&tuple! { x: 2i64 }));
    assert!(inv.inserted.contains(&tuple! { x: 1i64 }));

    let r_back = inv.apply(&r2).unwrap();
    assert_eq!(r_back, r1);
}

#[test]
fn test_compose() {
    let t = test_type();

    // R1: {1}
    let mut r1 = Relation::new(t.clone());
    r1.insert(tuple! { x: 1i64 }).unwrap();

    // R2: {2}
    let mut r2 = Relation::new(t.clone());
    r2.insert(tuple! { x: 2i64 }).unwrap();

    // R3: {3}
    let mut r3 = Relation::new(t.clone());
    r3.insert(tuple! { x: 3i64 }).unwrap();

    // D1: R1 -> R2 (del 1, ins 2)
    let d1 = Delta::between(&r1, &r2).unwrap();

    // D2: R2 -> R3 (del 2, ins 3)
    let d2 = Delta::between(&r2, &r3).unwrap();

    // D3: R1 -> R3 (should be del 1, ins 3)
    let d3 = d1.compose(&d2).unwrap();

    assert!(d3.deleted.contains(&tuple! { x: 1i64 }));
    assert!(!d3.deleted.contains(&tuple! { x: 2i64 })); // 2 was ins then del, so net zero

    assert!(d3.inserted.contains(&tuple! { x: 3i64 }));
    assert!(!d3.inserted.contains(&tuple! { x: 2i64 })); // 2 was ins then del, so net zero

    let r_final = d3.apply(&r1).unwrap();
    assert_eq!(r_final, r3);
}

#[test]
fn test_compose_overlapping() {
    // R1: {}
    // R2: {1} (D1: ins 1)
    // R3: {1, 2} (D2: ins 2)
    // D3 = D1 + D2 -> ins {1, 2}

    let t = test_type();
    let r1 = Relation::new(t.clone());

    let mut r2 = Relation::new(t.clone());
    r2.insert(tuple! { x: 1i64 }).unwrap();

    let mut r3 = Relation::new(t.clone());
    r3.insert(tuple! { x: 1i64 }).unwrap();
    r3.insert(tuple! { x: 2i64 }).unwrap();

    let d1 = Delta::between(&r1, &r2).unwrap();
    let d2 = Delta::between(&r2, &r3).unwrap();

    let d3 = d1.compose(&d2).unwrap();

    assert_eq!(d3.inserted.cardinality(), 2);
    assert!(d3.inserted.contains(&tuple! { x: 1i64 }));
    assert!(d3.inserted.contains(&tuple! { x: 2i64 }));

    let r_final = d3.apply(&r1).unwrap();
    assert_eq!(r_final, r3);
}
fn test_type_2() -> RelationType {
    RelationType::new(TupleType::new().with_attribute("y", ScalarType::Int))
}

#[test]
fn test_delta_new_type_mismatch() {
    let r1 = Relation::new(test_type());
    let r2 = Relation::new(test_type_2());

    let result = Delta::new(r1, r2);
    assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
}

#[test]
fn test_delta_between_type_mismatch() {
    let r1 = Relation::new(test_type());
    let r2 = Relation::new(test_type_2());

    let result = Delta::between(&r1, &r2);
    assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
}

#[test]
fn test_delta_apply_type_mismatch() {
    let r1 = Relation::new(test_type());
    let r2 = Relation::new(test_type());
    let r3 = Relation::new(test_type_2());

    let delta = Delta::between(&r1, &r2).unwrap();
    let result = delta.apply(&r3);
    assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
}

#[test]
fn test_delta_compose_type_mismatch() {
    let r1 = Relation::new(test_type());
    let r2 = Relation::new(test_type());
    let r3 = Relation::new(test_type_2());
    let r4 = Relation::new(test_type_2());

    let d1 = Delta::between(&r1, &r2).unwrap();
    let d2 = Delta::between(&r3, &r4).unwrap();

    let result = d1.compose(&d2);
    assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
}

mod additional_delta_tests {
    use super::super::*;
    use crate::types::{RelationType, ScalarType, TupleType};

    fn test_type_1() -> RelationType {
        RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int))
    }

    fn test_type_2() -> RelationType {
        RelationType::new(TupleType::new().with_attribute("y", ScalarType::Int))
    }

    #[test]
    fn test_delta_new_type_mismatch_internal() {
        let r1 = Relation::new(test_type_1());
        let r2 = Relation::new(test_type_2());

        let result = Delta::new(r1, r2);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }

    #[test]
    fn test_delta_between_type_mismatch_internal() {
        let r1 = Relation::new(test_type_1());
        let r2 = Relation::new(test_type_2());

        let result = Delta::between(&r1, &r2);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }

    #[test]
    fn test_delta_apply_type_mismatch_internal() {
        let r1 = Relation::new(test_type_1());
        let r2 = Relation::new(test_type_1());
        let r3 = Relation::new(test_type_2());

        let delta = Delta::between(&r1, &r2).unwrap();
        let result = delta.apply(&r3);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }

    #[test]
    fn test_delta_compose_type_mismatch_internal() {
        let r1 = Relation::new(test_type_1());
        let r2 = Relation::new(test_type_1());
        let r3 = Relation::new(test_type_2());
        let r4 = Relation::new(test_type_2());

        let d1 = Delta::between(&r1, &r2).unwrap();
        let d2 = Delta::between(&r3, &r4).unwrap();

        let result = d1.compose(&d2);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }
}
