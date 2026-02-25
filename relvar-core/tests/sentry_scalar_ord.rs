use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};
use std::cmp::Ordering;
use std::collections::BTreeSet;

#[test]
fn test_relation_scalar_ord_detects_content_difference() {
    let heading = TupleType::new().with_attribute("val", ScalarType::Int);
    let rel_type = RelationType::new(heading);

    // Relation 1: {val: 1}
    let mut r1 = Relation::new(rel_type.clone());
    r1.insert(tuple! { val: 1i64 }).unwrap();

    // Relation 2: {val: 2}
    let mut r2 = Relation::new(rel_type.clone());
    r2.insert(tuple! { val: 2i64 }).unwrap();

    assert_eq!(r1.cardinality(), 1);
    assert_eq!(r2.cardinality(), 1);
    assert_ne!(r1, r2); // Equality works

    let v1 = ScalarValue::Relation(r1);
    let v2 = ScalarValue::Relation(r2);

    // This assertion fails currently because cmp only checks cardinality/degree
    assert_ne!(
        v1.cmp(&v2),
        Ordering::Equal,
        "ScalarValue::cmp should distinguish different relations"
    );
}

#[test]
fn test_relation_scalar_btreeset_data_loss() {
    let heading = TupleType::new().with_attribute("val", ScalarType::Int);
    let rel_type = RelationType::new(heading);

    let mut r1 = Relation::new(rel_type.clone());
    r1.insert(tuple! { val: 1i64 }).unwrap();

    let mut r2 = Relation::new(rel_type.clone());
    r2.insert(tuple! { val: 2i64 }).unwrap();

    let v1 = ScalarValue::Relation(r1);
    let v2 = ScalarValue::Relation(r2);

    let mut set = BTreeSet::new();
    set.insert(v1);
    set.insert(v2);

    // This assertion fails currently because BTreeSet thinks they are equal keys
    assert_eq!(
        set.len(),
        2,
        "BTreeSet should contain both distinct relations"
    );
}
