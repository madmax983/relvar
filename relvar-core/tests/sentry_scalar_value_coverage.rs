use relvar_core::types::ScalarType;
use relvar_core::values::ScalarValue;

#[test]
fn test_scalar_value_select_type_mismatch() {
    let widget_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

    let res = ScalarValue::select(&widget_type, ScalarValue::String("not_an_int".to_string()));
    assert!(res.is_err());
}

#[test]
fn test_scalar_value_observer_not_user_defined() {
    let raw_int = ScalarValue::Int(42);
    let res = raw_int.observer();
    assert!(res.is_err());
}

#[test]
fn test_scalar_value_float_cmp() {
    let f1 = ScalarValue::Float(-10.0);
    let f2 = ScalarValue::Float(-1.0);
    let f3 = ScalarValue::Float(1.0);
    let f4 = ScalarValue::Float(10.0);

    // Test same sign negative
    assert!(f1 < f2);
    assert!(f2 > f1);

    // Test different sign
    assert!(f2 < f3);
    assert!(f3 > f2);

    // Test same sign positive
    assert!(f3 < f4);
    assert!(f4 > f3);
}

#[test]
fn test_scalar_value_hash_coverage() {
    use relvar_core::types::{RelationType, TupleType};
    use relvar_core::values::Relation;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn calculate_hash<T: Hash>(t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        s.finish()
    }

    let _ = calculate_hash(&ScalarValue::Int(42));
    let _ = calculate_hash(&ScalarValue::Float(42.0));
    let _ = calculate_hash(&ScalarValue::Float(f64::NAN));
    let _ = calculate_hash(&ScalarValue::String("hello".to_string()));

    // Stronger assertions (salvaged from swarm PR #1168): distinct values
    // of the same variant must hash distinctly.
    let bool_val = ScalarValue::Bool(true);
    let bytes_val = ScalarValue::Bytes(vec![1, 2, 3]);
    let rel_type = RelationType::new(TupleType::new().with_attribute("a", ScalarType::Int));
    let rel_val = ScalarValue::Relation(Relation::new(rel_type));

    assert_ne!(
        calculate_hash(&bool_val),
        calculate_hash(&ScalarValue::Bool(false))
    );
    assert_ne!(
        calculate_hash(&bytes_val),
        calculate_hash(&ScalarValue::Bytes(vec![1, 2]))
    );

    // Hash of Relation
    let _ = calculate_hash(&rel_val);
}

#[test]
fn test_scalar_value_hash_user_defined() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let widget_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    let val = ScalarValue::select(&widget_type, ScalarValue::Int(42)).unwrap();

    let mut s = DefaultHasher::new();
    val.hash(&mut s);
    let _ = s.finish();
}
