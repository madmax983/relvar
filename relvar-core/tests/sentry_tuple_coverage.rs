#[test]
fn test_tuple_error_display() {
    use relvar_core::values::TupleError;
    assert_eq!(
        format!("{}", TupleError::AttributeNotFound("attr1".to_string())),
        "Attribute 'attr1' not found in tuple type"
    );
    assert_eq!(
        format!(
            "{}",
            TupleError::TypeMismatch("attr1".to_string(), "Int".to_string(), "String".to_string())
        ),
        "Type mismatch for attribute 'attr1': expected \"Int\", got \"String\""
    );
    assert_eq!(
        format!("{}", TupleError::MissingValue("attr1".to_string())),
        "Missing value for attribute 'attr1'"
    );
}

#[test]
fn test_tuple_debug_clone_and_accessors() {
    use relvar_core::types::{ScalarType, TupleType};
    use relvar_core::values::{ScalarValue, Tuple, TupleError};
    use std::collections::HashMap;

    let err = TupleError::MissingValue("attr1".to_string());
    assert!(format!("{:?}", err).contains("MissingValue"));

    let tuple_type = TupleType::new()
        .with_attribute("val1", ScalarType::Int)
        .with_attribute("val2", ScalarType::Int);

    let mut values = HashMap::new();
    values.insert("val1".to_string(), ScalarValue::Int(10));
    values.insert("val2".to_string(), ScalarValue::Int(20));

    let tuple = Tuple::new(tuple_type.clone(), values).unwrap();
    let cloned_tuple = tuple.clone();

    assert_eq!(tuple, cloned_tuple);
    assert!(format!("{:?}", tuple).contains("val1"));

    assert_eq!(tuple.tuple_type(), &tuple_type);

    let mut names: Vec<String> = tuple.attribute_names().cloned().collect();
    names.sort();
    assert_eq!(names, vec!["val1".to_string(), "val2".to_string()]);

    let vals = tuple.values();
    assert_eq!(vals.get("val1"), Some(&ScalarValue::Int(10)));

    let into_vals = tuple.into_values();
    assert_eq!(into_vals.get("val2"), Some(&ScalarValue::Int(20)));
}

#[test]
fn test_tuple_conforms_to_mismatches() {
    use relvar_core::types::{ScalarType, TupleType};
    use relvar_core::values::{ScalarValue, Tuple};
    use std::collections::HashMap;

    let tuple_type1 = TupleType::new().with_attribute("val", ScalarType::Int);
    let tuple_type2 = TupleType::new()
        .with_attribute("val", ScalarType::Int)
        .with_attribute("val2", ScalarType::Int);
    let tuple_type3 = TupleType::new().with_attribute("val", ScalarType::String);

    let mut values = HashMap::new();
    values.insert("val".to_string(), ScalarValue::Int(10));

    let tuple = Tuple::new(tuple_type1, values).unwrap();
    assert!(!tuple.conforms_to(&tuple_type2)); // Mismatch degree
    assert!(!tuple.conforms_to(&tuple_type3)); // Mismatch type
}

#[test]
fn test_tuple_arc_serde_roundtrip() {
    use relvar_core::types::{ScalarType, TupleType};
    use relvar_core::values::{ScalarValue, Tuple};
    use std::collections::HashMap;

    let tuple_type = TupleType::new().with_attribute("val", ScalarType::Int);
    let mut values = HashMap::new();
    values.insert("val".to_string(), ScalarValue::Int(10));
    let tuple = Tuple::new(tuple_type, values).unwrap();

    let json = serde_json::to_string(&tuple).unwrap();
    let deserialized: Tuple = serde_json::from_str(&json).unwrap();
    assert_eq!(tuple, deserialized);
}
