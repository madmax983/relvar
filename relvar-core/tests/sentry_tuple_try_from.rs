use relvar_core::values::ScalarValue;
use std::convert::TryFrom;

#[test]
fn test_tuple_try_from_coverage() {
    let sv_int = ScalarValue::Int(42);
    let sv_float = ScalarValue::Float(42.0);
    let sv_string = ScalarValue::String("test".to_string());
    let sv_bool = ScalarValue::Bool(true);
    let sv_bytes = ScalarValue::Bytes(vec![1, 2, 3]);

    // Test TryFrom<ScalarValue> for owned values
    assert_eq!(i64::try_from(sv_int.clone()), Ok(42));
    assert_eq!(f64::try_from(sv_float.clone()), Ok(42.0));
    assert_eq!(String::try_from(sv_string.clone()), Ok("test".to_string()));
    assert_eq!(bool::try_from(sv_bool.clone()), Ok(true));
    assert_eq!(Vec::<u8>::try_from(sv_bytes.clone()), Ok(vec![1, 2, 3]));

    // Test error cases for owned TryFrom
    assert!(i64::try_from(sv_string.clone()).is_err());
    assert!(f64::try_from(sv_int.clone()).is_err());
    assert!(String::try_from(sv_float.clone()).is_err());
    assert!(bool::try_from(sv_string.clone()).is_err());
    assert!(Vec::<u8>::try_from(sv_int.clone()).is_err());

    // Test TryFrom<&ScalarValue> for borrowed values
    assert_eq!(i64::try_from(&sv_int), Ok(42));
    assert_eq!(f64::try_from(&sv_float), Ok(42.0));
    assert_eq!(String::try_from(&sv_string), Ok("test".to_string()));
    assert_eq!(bool::try_from(&sv_bool), Ok(true));
    assert_eq!(Vec::<u8>::try_from(&sv_bytes), Ok(vec![1, 2, 3]));

    // Test error cases for borrowed TryFrom
    assert!(i64::try_from(&sv_string).is_err());
    assert!(f64::try_from(&sv_int).is_err());
    assert!(String::try_from(&sv_float).is_err());
    assert!(bool::try_from(&sv_string).is_err());
    assert!(Vec::<u8>::try_from(&sv_int).is_err());

    // Test TryFrom<&ScalarValue> for &str (special case in source)
    assert_eq!(<&str>::try_from(&sv_string), Ok("test"));
    assert!(<&str>::try_from(&sv_int).is_err());
}

#[test]
fn test_tuple_from_coverage() {
    // Tests lines 507-510, 517-520
    assert_eq!(
        ScalarValue::from("hello"),
        ScalarValue::String("hello".to_string())
    );
    assert_eq!(ScalarValue::from(true), ScalarValue::Bool(true));
    assert_eq!(ScalarValue::from(false), ScalarValue::Bool(false));
    assert_eq!(
        ScalarValue::from(vec![1, 2, 3]),
        ScalarValue::Bytes(vec![1, 2, 3])
    );
    assert_eq!(ScalarValue::from(42i64), ScalarValue::Int(42));
    assert_eq!(ScalarValue::from(42.0f64), ScalarValue::Float(42.0));
    assert_eq!(
        ScalarValue::from("hello".to_string()),
        ScalarValue::String("hello".to_string())
    );
}
