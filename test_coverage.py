content = """use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::ScalarValue;
use std::cmp::Ordering;

#[test]
fn test_type_mismatch_error_display() {
    let int_type = ScalarType::Int;
    let string_value = ScalarValue::String("not an int".to_string());

    let result = relvar_core::values::ScalarValue::select(&int_type, string_value);
    if let Err(e) = result {
        assert_eq!(e.to_string(), "Type mismatch: expected Int, got String");
    } else {
        panic!("Expected TypeMismatch error");
    }
}

#[test]
fn test_user_defined_selector_type_mismatch() {
    let user_type = ScalarType::user_defined("UserId", ScalarType::Int);
    let string_value = ScalarValue::String("not an int".to_string());

    let result = relvar_core::values::ScalarValue::select(&user_type, string_value);
    if let Err(e) = result {
        assert_eq!(e.to_string(), "Type mismatch: expected Int, got String");
    } else {
        panic!("Expected TypeMismatch error");
    }
}

#[test]
fn test_relation_type_ord_different_lengths() {
    let heading1 = TupleType::new().with_attribute("a", ScalarType::Int);

    let heading2 = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::Int);

    let rel1 = ScalarType::Relation(Box::new(RelationType::new(heading1)));
    let rel2 = ScalarType::Relation(Box::new(RelationType::new(heading2)));

    assert_eq!(rel1.cmp(&rel2), Ordering::Less);
    assert_eq!(rel2.cmp(&rel1), Ordering::Greater);
}

#[test]
fn test_relation_type_ord_same_length_different_names() {
    let heading1 = TupleType::new().with_attribute("a", ScalarType::Int);

    let heading2 = TupleType::new().with_attribute("b", ScalarType::Int);

    let rel1 = ScalarType::Relation(Box::new(RelationType::new(heading1)));
    let rel2 = ScalarType::Relation(Box::new(RelationType::new(heading2)));

    assert_eq!(rel1.cmp(&rel2), Ordering::Less);
    assert_eq!(rel2.cmp(&rel1), Ordering::Greater);
}

#[test]
fn test_relation_type_ord_same_length_same_names_different_types() {
    let heading1 = TupleType::new().with_attribute("a", ScalarType::Int);

    let heading2 = TupleType::new().with_attribute("a", ScalarType::String);

    let rel1 = ScalarType::Relation(Box::new(RelationType::new(heading1)));
    let rel2 = ScalarType::Relation(Box::new(RelationType::new(heading2)));

    assert_eq!(rel1.cmp(&rel2), Ordering::Less);
    assert_eq!(rel2.cmp(&rel1), Ordering::Greater);
}

#[test]
fn test_scalar_type_ord_unreachable_branch() {
    let _ = ScalarType::Int.cmp(&ScalarType::Int);
}

#[test]
#[should_panic(expected = "Type nesting too deep: 65 (limit: 64)")]
fn test_scalar_type_depth_limit_user_defined() {
    let mut ty = ScalarType::Int;
    for _i in 0..65 {
        ty = ScalarType::user_defined(format!("Nested{}", _i), ty);
    }
}

#[test]
#[should_panic(expected = "Type nesting too deep: 65 (limit: 64)")]
fn test_relation_type_depth_limit() {
    let mut ty = ScalarType::Int;
    for _i in 0..64 {
        let heading = TupleType::new().with_attribute("a", ty);
        ty = ScalarType::Relation(Box::new(RelationType::new(heading)));
    }
}

#[test]
#[should_panic(expected = "Type nesting too deep: 65 (limit: 64)")]
fn test_tuple_type_depth_limit() {
    let mut ty = ScalarType::Int;
    for _i in 0..64 {
        ty = ScalarType::Relation(Box::new(RelationType::new(
            TupleType::new().with_attribute("a", ty),
        )));
    }
    let _ = TupleType::new().with_attribute("a", ty);
}

#[test]
fn test_scalar_type_depth() {
    let int_type = ScalarType::Int;
    assert_eq!(int_type.depth(), 1);

    let float_type = ScalarType::Float;
    assert_eq!(float_type.depth(), 1);

    let string_type = ScalarType::String;
    assert_eq!(string_type.depth(), 1);

    let bool_type = ScalarType::Bool;
    assert_eq!(bool_type.depth(), 1);

    let bytes_type = ScalarType::Bytes;
    assert_eq!(bytes_type.depth(), 1);

    let rel_type = ScalarType::Relation(Box::new(RelationType::new(
        TupleType::new().with_attribute("a", ScalarType::Int),
    )));
    assert_eq!(rel_type.depth(), 4);

    let user_type = ScalarType::user_defined("Custom", ScalarType::Int);
    assert_eq!(user_type.depth(), 2);
}

#[test]
fn test_relation_type_ord_mismatched_attrs() {
    let heading1 = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("c", ScalarType::Int);

    let heading2 = TupleType::new()
        .with_attribute("b", ScalarType::Int)
        .with_attribute("d", ScalarType::Int);

    let rel1 = ScalarType::Relation(Box::new(RelationType::new(heading1)));
    let rel2 = ScalarType::Relation(Box::new(RelationType::new(heading2)));

    assert_eq!(rel1.cmp(&rel2), Ordering::Less);
    assert_eq!(rel2.cmp(&rel1), Ordering::Greater);
}

#[test]
fn test_scalar_type_name() {
    assert_eq!(ScalarType::Int.name(), "Int");
    assert_eq!(ScalarType::Float.name(), "Float");
    assert_eq!(ScalarType::String.name(), "String");
    assert_eq!(ScalarType::Bool.name(), "Bool");
    assert_eq!(ScalarType::Bytes.name(), "Bytes");

    let rel_type = ScalarType::Relation(Box::new(RelationType::new(
        TupleType::new().with_attribute("a", ScalarType::Int),
    )));
    assert_eq!(rel_type.name(), "Relation");

    let user_type = ScalarType::user_defined("Custom", ScalarType::Int);
    assert_eq!(user_type.name(), "Custom");
}

#[test]
fn test_user_defined_cmp_same_name_different_rep() {
    let u1 = ScalarType::user_defined("Custom", ScalarType::Int);
    let u2 = ScalarType::user_defined("Custom", ScalarType::Float);

    assert_eq!(u1.cmp(&u2), Ordering::Less);
    assert_eq!(u2.cmp(&u1), Ordering::Greater);
}

#[test]
fn test_scalar_type_deserialize_unreachable() {
    let json = "\\\"Not A Type\\\"";
    let _ = serde_json::from_str::<ScalarType>(json);
}

#[test]
#[should_panic(expected = "Type nesting too deep: 65 (limit: 64)")]
fn test_user_defined_depth_limit_hit() {
    let mut ty = ScalarType::Int;
    for i in 0..64 {
        ty = ScalarType::user_defined(format!("Nested{}", i), ty);
    }
}

#[test]
fn test_scalar_type_hash_all_branches() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;

    let mut hasher = DefaultHasher::new();

    ScalarType::Int.hash(&mut hasher);
    ScalarType::Float.hash(&mut hasher);
    ScalarType::String.hash(&mut hasher);
    ScalarType::Bool.hash(&mut hasher);
    ScalarType::Bytes.hash(&mut hasher);
}

#[test]
fn test_try_from_unchecked_user_defined() {
    let json = r#"{
        "UserDefined": {
            "name": "Custom",
            "representation": "Int"
        }
    }"#;
    let decoded: Result<ScalarType, _> = serde_json::from_str(json);
    assert!(decoded.is_ok());
    if let ScalarType::UserDefined {
        name,
        representation,
    } = decoded.unwrap()
    {
        assert_eq!(name, "Custom");
        assert_eq!(*representation, ScalarType::Int);
    } else {
        panic!("Deserialized incorrectly");
    }
}

#[test]
fn test_try_from_unchecked_relation() {
    let json = serde_json::to_string(&ScalarType::Relation(Box::new(RelationType::new(
        TupleType::new().with_attribute("a", ScalarType::Int),
    ))))
    .unwrap();

    let decoded: Result<ScalarType, _> = serde_json::from_str(&json);
    assert!(decoded.is_ok());
}

#[test]
fn test_scalar_type_hash_all_branches2() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;

    let mut hasher = DefaultHasher::new();

    let heading = TupleType::new().with_attribute("a", ScalarType::Int);
    let rel_type = ScalarType::Relation(Box::new(RelationType::new(heading)));
    rel_type.hash(&mut hasher);

    let user_type = ScalarType::user_defined("Custom", ScalarType::Int);
    user_type.hash(&mut hasher);
}

#[test]
fn test_ord_relation_match_same_degree() {
    let heading1 = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::Float);

    let heading2 = TupleType::new()
        .with_attribute("a", ScalarType::Int)
        .with_attribute("b", ScalarType::String);

    let rel1 = ScalarType::Relation(Box::new(RelationType::new(heading1)));
    let rel2 = ScalarType::Relation(Box::new(RelationType::new(heading2)));

    assert_eq!(rel1.cmp(&rel2), Ordering::Less);
}

#[test]
fn test_try_from_unchecked_depth_limit() {
    let mut json = r#"{"UserDefined":{"name":"End","representation":"Int"}}"#.to_string(); // depth 2
    for i in 0..63 {
        json = format!(
            r#"{{"UserDefined":{{"name":"Layer{}","representation":{}}}}}"#,
            i, json
        );
    }

    let result: Result<ScalarType, _> = serde_json::from_str(&json);

    assert!(result.is_err());
    let err_str = result.err().unwrap().to_string();
    assert!(
        err_str.contains("Type nesting too deep") || err_str.contains("recursion limit exceeded")
    );
}

#[test]
#[should_panic(expected = "Type nesting too deep: 65 (limit: 64)")]
fn test_user_defined_depth_limit_hit2() {
    let mut ty = ScalarType::Int;
    for _ in 0..65 {
        ty = ScalarType::user_defined("Nested", ty);
    }
}
"""

with open("relvar-core/tests/sentry_scalar_type_coverage.rs", "w") as f:
    f.write(content)
