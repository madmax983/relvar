use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use relvar_core::values::{ScalarValue, TupleError};
use std::collections::hash_map::DefaultHasher;
use std::convert::TryFrom;
use std::hash::{Hash, Hasher};

#[test]
fn test_scalar_value_conversions() {
    // Int
    let int_val = ScalarValue::Int(42);
    assert_eq!(i64::try_from(int_val.clone()).unwrap(), 42);
    assert_eq!(i64::try_from(&int_val).unwrap(), 42);
    assert!(f64::try_from(int_val.clone()).is_err());
    assert!(f64::try_from(&int_val).is_err());

    // Float
    let float_val = ScalarValue::Float(3.5);
    assert_eq!(f64::try_from(float_val.clone()).unwrap(), 3.5);
    assert_eq!(f64::try_from(&float_val).unwrap(), 3.5);
    assert!(i64::try_from(float_val.clone()).is_err());
    assert!(i64::try_from(&float_val).is_err());

    // String
    let str_val = ScalarValue::String("hello".to_string());
    assert_eq!(String::try_from(str_val.clone()).unwrap(), "hello");
    assert_eq!(String::try_from(&str_val).unwrap(), "hello");
    assert!(i64::try_from(str_val.clone()).is_err());
    assert!(i64::try_from(&str_val).is_err());

    // From implementations for String and &str
    let s1 = ScalarValue::from("hello".to_string());
    assert!(matches!(s1, ScalarValue::String(_)));
    let s2 = ScalarValue::from("hello");
    assert!(matches!(s2, ScalarValue::String(_)));

    // Bool
    let bool_val = ScalarValue::Bool(true);
    assert!(bool::try_from(bool_val.clone()).unwrap());
    assert!(bool::try_from(&bool_val).unwrap());
    assert!(i64::try_from(bool_val.clone()).is_err());
    assert!(i64::try_from(&bool_val).is_err());

    let b1 = ScalarValue::from(true);
    assert!(matches!(b1, ScalarValue::Bool(true)));

    // Bytes
    let bytes_val = ScalarValue::Bytes(vec![1, 2, 3]);
    assert_eq!(
        Vec::<u8>::try_from(bytes_val.clone()).unwrap(),
        vec![1, 2, 3]
    );
    assert_eq!(Vec::<u8>::try_from(&bytes_val).unwrap(), vec![1, 2, 3]);
    assert!(i64::try_from(bytes_val.clone()).is_err());
    assert!(i64::try_from(&bytes_val).is_err());

    let bytes1 = ScalarValue::from(vec![1, 2, 3]);
    assert!(matches!(bytes1, ScalarValue::Bytes(_)));
}

#[test]
fn test_tuple_methods() {
    let mut t = tuple! { id: 1i64, active: true };
    assert_eq!(t.get_typed::<i64>("id"), Some(1));
    assert_eq!(t.get_typed::<bool>("active"), Some(true));
    assert_eq!(t.get_typed::<String>("id"), None);

    let correct_heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("active", ScalarType::Bool);
    assert!(t.conforms_to(&correct_heading));

    let wrong_heading = TupleType::new()
        .with_attribute("id", ScalarType::String)
        .with_attribute("active", ScalarType::Bool);
    assert!(!t.conforms_to(&wrong_heading));

    let wrong_heading2 = TupleType::new().with_attribute("nonexistent", ScalarType::Int);
    assert!(!t.conforms_to(&wrong_heading2));

    // Set operations
    // Set a nonexistent field should fail
    assert!(matches!(
        t.set("nonexistent".to_string(), ScalarValue::Int(2)),
        Err(TupleError::AttributeNotFound(_))
    ));

    // Set field with wrong type should fail
    assert!(matches!(
        t.set("id".to_string(), ScalarValue::String("2".to_string())),
        Err(TupleError::TypeMismatch(_, _, _))
    ));

    // Hash
    let mut hasher = DefaultHasher::new();
    t.hash(&mut hasher);
    let _hash = hasher.finish();
}
