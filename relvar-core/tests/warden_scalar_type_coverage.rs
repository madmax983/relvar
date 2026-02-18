use relvar_core::types::ScalarType;
use serde_json;

#[test]
fn test_scalar_type_deserialization_coverage() {
    // Construct a JSON array containing all variants of ScalarTypeUnchecked
    let json = r#"[
        "Int",
        "Float",
        "String",
        "Bool",
        "Bytes",
        {"Relation": {"heading": {"attributes": {}}}},
        {"UserDefined": {"name": "MyType", "representation": "Int"}}
    ]"#;

    // Deserialize into Vec<ScalarType>
    // This will trigger TryFrom<ScalarTypeUnchecked> for each element
    let types: Vec<ScalarType> = serde_json::from_str(json).expect("Deserialization failed");

    // Verify correct types were created
    assert_eq!(types.len(), 7);
    assert_eq!(types[0], ScalarType::Int);
    assert_eq!(types[1], ScalarType::Float);
    assert_eq!(types[2], ScalarType::String);
    assert_eq!(types[3], ScalarType::Bool);
    assert_eq!(types[4], ScalarType::Bytes);

    // Relation type check
    if let ScalarType::Relation(rel_type) = &types[5] {
        assert_eq!(rel_type.heading().degree(), 0);
    } else {
        panic!("Expected Relation type at index 5");
    }

    // UserDefined type check
    if let ScalarType::UserDefined {
        name,
        representation,
    } = &types[6]
    {
        assert_eq!(name, "MyType");
        assert_eq!(**representation, ScalarType::Int);
    } else {
        panic!("Expected UserDefined type at index 6");
    }
}
