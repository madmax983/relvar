use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

#[test]
fn test_tclose_simple_chain() {
    // A -> B -> C
    let heading = TupleType::new()
        .with_attribute("start", ScalarType::Int)
        .with_attribute("end", ScalarType::Int);
    let mut r = Relation::new(RelationType::new(heading));

    r.insert(tuple! { start: 1i64, end: 2i64 }).unwrap();
    r.insert(tuple! { start: 2i64, end: 3i64 }).unwrap();

    let closure = r.tclose("start", "end").unwrap();

    // Expected: (1,2), (2,3), (1,3)
    assert_eq!(closure.cardinality(), 3);
    assert!(closure.contains(&tuple! { start: 1i64, end: 3i64 }));
}

#[test]
fn test_tclose_cycle() {
    // A -> B -> A
    let heading = TupleType::new()
        .with_attribute("n1", ScalarType::String)
        .with_attribute("n2", ScalarType::String);
    let mut r = Relation::new(RelationType::new(heading));

    r.insert(tuple! { n1: "A", n2: "B" }).unwrap();
    r.insert(tuple! { n1: "B", n2: "A" }).unwrap();

    let closure = r.tclose("n1", "n2").unwrap();

    // Expected:
    // (A, B) - initial
    // (B, A) - initial
    // (A, A) - via A->B->A
    // (B, B) - via B->A->B
    assert_eq!(closure.cardinality(), 4);
    assert!(closure.contains(&tuple! { n1: "A", n2: "A" }));
    assert!(closure.contains(&tuple! { n1: "B", n2: "B" }));
}

#[test]
fn test_tclose_disconnected() {
    // 1->2, 3->4
    let heading = TupleType::new()
        .with_attribute("x", ScalarType::Int)
        .with_attribute("y", ScalarType::Int);
    let mut r = Relation::new(RelationType::new(heading));

    r.insert(tuple! { x: 1i64, y: 2i64 }).unwrap();
    r.insert(tuple! { x: 3i64, y: 4i64 }).unwrap();

    let closure = r.tclose("x", "y").unwrap();

    // No new paths
    assert_eq!(closure.cardinality(), 2);
}

#[test]
fn test_tclose_invalid_degree() {
    let heading = TupleType::new().with_attribute("x", ScalarType::Int);
    let r = Relation::new(RelationType::new(heading));

    assert!(r.tclose("x", "x").is_err());
}

#[test]
fn test_tclose_type_mismatch() {
    let heading = TupleType::new()
        .with_attribute("x", ScalarType::Int)
        .with_attribute("y", ScalarType::String);
    let mut r = Relation::new(RelationType::new(heading));
    r.insert(tuple! { x: 1i64, y: "a" }).unwrap();

    assert!(r.tclose("x", "y").is_err());
}
