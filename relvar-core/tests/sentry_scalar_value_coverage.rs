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
    let _ = calculate_hash(&ScalarValue::Bool(true));
    let _ = calculate_hash(&ScalarValue::Bytes(vec![1, 2, 3]));
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

#[test]
fn test_scalar_value_unchecked_user_defined_invalid_type_def() {
    let json = r#"
        {"UserDefined": {
            "type_def": "Int",
            "value": {"Int": 42}
        }}
    "#;
    let decoded: Result<ScalarValue, _> = serde_json::from_str(json);
    assert!(decoded.is_err());
    let err_msg = decoded.unwrap_err().to_string();
    assert!(err_msg.contains("Invalid UserDefined value: type definition must be UserDefined"));
}

#[test]
fn test_scalar_value_unchecked_user_defined_type_mismatch() {
    let json = r#"
        {"UserDefined": {
            "type_def": {"UserDefined": {"name": "WidgetId", "representation": "Int"}},
            "value": {"String": "42"}
        }}
    "#;
    let decoded: Result<ScalarValue, _> = serde_json::from_str(json);
    assert!(decoded.is_err());
    let err_msg = decoded.unwrap_err().to_string();
    assert!(err_msg.contains("Type mismatch in UserDefined value: type definition expects Int, but value is String"));
}
