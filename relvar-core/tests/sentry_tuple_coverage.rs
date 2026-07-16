use relvar_core::values::ScalarValue;
use std::convert::TryFrom;

#[test]
fn test_try_from_scalar_value() {
    let int_val = ScalarValue::Int(42);
    let float_val = ScalarValue::Float(1.23);
    let str_val = ScalarValue::String("hello".to_string());
    let bool_val = ScalarValue::Bool(true);
    let bytes_val = ScalarValue::Bytes(vec![1, 2, 3]);

    // Test success cases for consuming TryFrom
    assert_eq!(i64::try_from(int_val.clone()), Ok(42));
    assert_eq!(f64::try_from(float_val.clone()), Ok(1.23));
    assert_eq!(String::try_from(str_val.clone()), Ok("hello".to_string()));
    assert_eq!(bool::try_from(bool_val.clone()), Ok(true));
    assert_eq!(Vec::<u8>::try_from(bytes_val.clone()), Ok(vec![1, 2, 3]));

    // Test success cases for borrowing TryFrom
    assert_eq!(i64::try_from(&int_val), Ok(42));
    assert_eq!(f64::try_from(&float_val), Ok(1.23));
    assert_eq!(String::try_from(&str_val), Ok("hello".to_string()));
    assert_eq!(bool::try_from(&bool_val), Ok(true));
    assert_eq!(Vec::<u8>::try_from(&bytes_val), Ok(vec![1, 2, 3]));
}
