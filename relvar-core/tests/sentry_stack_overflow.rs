use relvar_core::values::ScalarValue;
use relvar_core::types::ScalarType;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

#[test]
fn test_deeply_nested_drop_should_not_overflow() {
    let mut val = ScalarValue::Int(0);
    let type_def = ScalarType::user_defined("RecursiveType", ScalarType::Int);

    // 100,000 layers should be enough to blow the stack on most systems if recursion is present
    for _ in 0..100000 {
        val = ScalarValue::UserDefined {
            type_def: type_def.clone(),
            value: Box::new(val),
        };
    }
}

#[test]
fn test_deeply_nested_eq_should_not_overflow() {
    let mut val1 = ScalarValue::Int(0);
    let mut val2 = ScalarValue::Int(0);
    let type_def = ScalarType::user_defined("RecursiveType", ScalarType::Int);

    for _ in 0..100000 {
        val1 = ScalarValue::UserDefined {
            type_def: type_def.clone(),
            value: Box::new(val1),
        };
        val2 = ScalarValue::UserDefined {
            type_def: type_def.clone(),
            value: Box::new(val2),
        };
    }

    assert_eq!(val1, val2);
}

#[test]
fn test_deeply_nested_hash_should_not_overflow() {
    let mut val = ScalarValue::Int(0);
    let type_def = ScalarType::user_defined("RecursiveType", ScalarType::Int);

    for _ in 0..100000 {
        val = ScalarValue::UserDefined {
            type_def: type_def.clone(),
            value: Box::new(val),
        };
    }

    let mut hasher = DefaultHasher::new();
    val.hash(&mut hasher);
    let _hash = hasher.finish();
}

#[test]
fn test_deeply_nested_ord_should_not_overflow() {
    let mut val1 = ScalarValue::Int(0);
    let mut val2 = ScalarValue::Int(0);
    let type_def = ScalarType::user_defined("RecursiveType", ScalarType::Int);

    for _ in 0..100000 {
        val1 = ScalarValue::UserDefined {
            type_def: type_def.clone(),
            value: Box::new(val1),
        };
        val2 = ScalarValue::UserDefined {
            type_def: type_def.clone(),
            value: Box::new(val2),
        };
    }

    // This triggers Ord::cmp
    assert!(val1 <= val2);
}
