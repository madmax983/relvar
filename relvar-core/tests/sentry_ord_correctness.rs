use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};
use std::collections::BTreeSet;

#[test]
fn test_relation_ord_fixed() {
    let heading = TupleType::new().with_attribute("a", ScalarType::Int);
    let rel_type = RelationType::new(heading);

    let mut r1 = Relation::new(rel_type.clone());
    r1.insert(tuple! { a: 1i64 }).unwrap();

    let mut r2 = Relation::new(rel_type.clone());
    r2.insert(tuple! { a: 2i64 }).unwrap();

    let v1 = ScalarValue::Relation(r1);
    let v2 = ScalarValue::Relation(r2);

    // They are NOT equal
    assert_ne!(v1, v2);

    // With the fix, they should NOT compare equal
    assert_ne!(v1.cmp(&v2), std::cmp::Ordering::Equal);

    // If we put them in a BTreeSet, both should be kept.
    let mut set = BTreeSet::new();
    set.insert(v1.clone());
    set.insert(v2.clone());

    assert_eq!(
        set.len(),
        2,
        "BTreeSet should keep both values as they are distinct"
    );
}
