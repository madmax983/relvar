use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;

#[test]
fn test_semijoin_keys_eq_direct() {
    let heading_c = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::String)
        .with_attribute("c", ScalarType::Int);

    let mut rel_c1 = Relation::new(RelationType::new(heading_c.clone()));
    rel_c1.insert(tuple! { a: 1i64, b: "x", c: 1i64 }).unwrap();
    rel_c1.insert(tuple! { a: 1i64, b: "y", c: 2i64 }).unwrap();

    let heading_c2 = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::String)
        .with_attribute("d", ScalarType::Int);

    let mut rel_c2 = Relation::new(RelationType::new(heading_c2));
    rel_c2.insert(tuple! { a: 1i64, b: "x", d: 3i64 }).unwrap();
}

#[test]
fn test_semijoin_keys_eq() {
    let heading_c = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::String)
        .with_attribute("c", ScalarType::Int);

    let mut rel_c1 = Relation::new(RelationType::new(heading_c.clone()));
    rel_c1.insert(tuple! { a: 1i64, b: "x", c: 1i64 }).unwrap();
    rel_c1.insert(tuple! { a: 1i64, b: "y", c: 2i64 }).unwrap();

    let heading_c2 = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::String)
        .with_attribute("d", ScalarType::Int);

    let mut rel_c2 = Relation::new(RelationType::new(heading_c2));
    rel_c2.insert(tuple! { a: 1i64, b: "x", d: 3i64 }).unwrap();

    // Semijoin will compare keys. They have "a" and "b" in common.
    let result = rel_c1.semijoin(&rel_c2);
    assert_eq!(result.cardinality(), 1);

    let result_diff = rel_c1.semidifference(&rel_c2);
    assert_eq!(result_diff.cardinality(), 1);

    let mut other_rel = Relation::new(RelationType::new(heading_c.clone()));
    other_rel
        .insert(tuple! { a: 1i64, b: "z", c: 1i64 })
        .unwrap();
    let res = rel_c1.semijoin(&other_rel);
    assert_eq!(res.cardinality(), 0);
}

#[test]
fn test_semijoin_hash_collision() {
    let heading_a = TupleType::new()
        .with_attribute("a", ScalarType::String)
        .with_attribute("b", ScalarType::String);

    let heading_b = TupleType::new()
        .with_attribute("a", ScalarType::String)
        .with_attribute("b", ScalarType::String)
        .with_attribute("c", ScalarType::String);

    let mut rel_a = Relation::new(RelationType::new(heading_a.clone()));

    for i in 0..1000 {
        rel_a
            .insert(tuple! { a: format!("val{}", i % 100), b: format!("other{}", i % 10) })
            .unwrap();
    }

    let mut rel_b = Relation::new(RelationType::new(heading_b));
    for i in 0..1000 {
        rel_b
            .insert(tuple! { a: format!("val{}", i % 10), b: format!("other{}", i % 10), c: "c" })
            .unwrap();
    }

    let result = rel_a.semijoin(&rel_b);
    assert!(result.cardinality() > 0);
}
