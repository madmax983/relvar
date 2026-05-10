use relvar_core::types::ScalarType;
use relvar_core::values::ScalarValue;

// Tests missing branches in relvar-core/src/types/scalar.rs

#[test]
fn test_scalar_type_ord_unreachable() {
    // Tests `_ => unreachable!()` in `Ord for ScalarType`
    let _ = ScalarType::Int.cmp(&ScalarType::Float);
}

#[test]
fn test_scalar_type_ord_user_defined() {
    // Tests `ScalarType::UserDefined` vs `ScalarType::UserDefined` Ord logic
    let u1 = ScalarType::user_defined("A", ScalarType::Int);
    let u2 = ScalarType::user_defined("A", ScalarType::Int);
    let u3 = ScalarType::user_defined("B", ScalarType::Int);
    let u4 = ScalarType::user_defined("A", ScalarType::Float);

    assert_eq!(u1.cmp(&u2), std::cmp::Ordering::Equal);
    assert_eq!(u1.cmp(&u3), std::cmp::Ordering::Less);
    assert_eq!(u3.cmp(&u1), std::cmp::Ordering::Greater);
    assert_eq!(u1.cmp(&u4), std::cmp::Ordering::Less);
}

#[test]
fn test_scalar_type_serialize_relation_and_user_defined() {
    use relvar_core::types::{RelationType, TupleType};
    let rel_ty = ScalarType::Relation(Box::new(RelationType::new(TupleType::new())));
    let _ = serde_json::to_string(&rel_ty).unwrap();

    let u_ty = ScalarType::user_defined("U", ScalarType::Int);
    let _ = serde_json::to_string(&u_ty).unwrap();
}

#[test]
fn test_scalar_type_user_defined_examples() {
    let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    assert_eq!(widget_id_type.name(), "WidgetId");
}

// Tests missing branches in relvar-core/src/values/scalar.rs

#[test]
fn test_scalar_value_ord_unreachable() {
    let _ = ScalarValue::Int(1).cmp(&ScalarValue::Float(1.0));
}

#[test]
fn test_scalar_value_eq_unreachable() {
    let _ = ScalarValue::Int(1).eq(&ScalarValue::Float(1.0));
}

#[test]
fn test_scalar_value_cmp_floats_bits_and_nan() {
    let zero = ScalarValue::Float(0.0);
    let neg_zero = ScalarValue::Float(-0.0);
    assert_ne!(zero, neg_zero);
    assert_ne!(zero.cmp(&neg_zero), std::cmp::Ordering::Equal);

    let nan1 = ScalarValue::Float(f64::NAN);
    let nan2 = ScalarValue::Float(f64::NAN);
    let inf = ScalarValue::Float(f64::INFINITY);

    assert_eq!(nan1, nan2);
    assert_ne!(nan1, inf);

    assert_eq!(nan1.cmp(&nan2), std::cmp::Ordering::Equal);
    assert_eq!(nan1.cmp(&inf), std::cmp::Ordering::Greater);
    assert_eq!(inf.cmp(&nan1), std::cmp::Ordering::Less);
}

#[test]
fn test_scalar_value_hash_string_bool_bytes() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut s1 = DefaultHasher::new();
    ScalarValue::String("test".to_string()).hash(&mut s1);
    let _ = s1.finish();

    let mut s2 = DefaultHasher::new();
    ScalarValue::Bool(true).hash(&mut s2);
    let _ = s2.finish();

    let mut s3 = DefaultHasher::new();
    ScalarValue::Bytes(vec![1, 2]).hash(&mut s3);
    let _ = s3.finish();
}

#[test]
fn test_scalar_value_cmp_relation_branches() {
    use relvar_core::types::{RelationType, TupleType};
    use relvar_core::values::{Relation, Tuple};

    let rel_type = RelationType::new(TupleType::new());
    let mut r1 = Relation::new(rel_type.clone());
    let r2 = Relation::new(rel_type.clone());
    r1.insert(Tuple::new(TupleType::new(), vec![]).unwrap())
        .unwrap();
    let v1 = ScalarValue::Relation(r1);
    let v2 = ScalarValue::Relation(r2.clone());
    assert_eq!(v1.cmp(&v2), std::cmp::Ordering::Greater);

    let mut tt1 = TupleType::new();
    tt1 = tt1.with_attribute("a", ScalarType::Int);
    let r3 = Relation::new(RelationType::new(tt1));
    let v3 = ScalarValue::Relation(r3);
    assert_eq!(v3.cmp(&v2), std::cmp::Ordering::Greater);
    assert_eq!(v2.cmp(&v3), std::cmp::Ordering::Less);
}

#[test]
fn test_scalar_value_user_defined_loop_other() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let u1 = ScalarType::user_defined("U1", ScalarType::Int);

    // Coverage for `other => return other` in hash
    let v1 = ScalarValue::select(&u1, ScalarValue::Int(42)).unwrap();
    let mut s = DefaultHasher::new();
    v1.hash(&mut s);
    let _ = s.finish();
}
