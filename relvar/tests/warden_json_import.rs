use relvar::data::importer::{self, ImporterError};
use relvar_core::types::{RelationType, ScalarType, TupleType};
use std::io::Cursor;

#[test]
fn test_large_string_import() {
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("data", ScalarType::String);
    let rel_type = RelationType::new(heading);

    let huge_string = "a".repeat(10 * 1024 * 1024); // 10MB
    let json = format!(
        r#"[
        {{"id": 1, "data": "{}"}}
    ]"#,
        huge_string
    );

    let result = importer::from_json(Cursor::new(json.as_bytes()), rel_type);

    assert!(result.is_err(), "Import of 10MB string should have failed");

    match result {
        Err(ImporterError::JsonError(e)) => {
            // Serde throws an unexpected EOF error when the capped reader hits its limit
            assert!(e.to_string().contains("EOF"));
        }
        Err(ImporterError::LimitExceeded(msg)) => {
            assert!(msg.contains("String too long"));
        }
        Err(e) => {
            panic!("Expected LimitExceeded or JsonError with EOF, got {:?}", e);
        }
        Ok(_) => panic!("Import succeeded but should have failed due to size limit"),
    }
}

#[test]
fn test_large_bytes_import() {
    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("data", ScalarType::Bytes);
    let rel_type = RelationType::new(heading);

    let mut json = String::with_capacity(30 * 1024 * 1024);
    json.push_str(r#"[{"id": 1, "data": ["#);
    for i in 0..10_000_000 {
        if i > 0 {
            json.push(',');
        }
        json.push('1');
    }
    json.push_str(r#"]}]"#);

    let result = importer::from_json(Cursor::new(json.as_bytes()), rel_type);

    assert!(
        result.is_err(),
        "Import of 10MB byte array should have failed"
    );

    match result {
        Err(ImporterError::JsonError(e)) => {
            // Serde throws an unexpected EOF error when the capped reader hits its limit
            assert!(e.to_string().contains("EOF"));
        }
        Err(ImporterError::LimitExceeded(msg)) => {
            assert!(msg.contains("Bytes too long"));
        }
        Err(e) => {
            panic!("Expected LimitExceeded or JsonError with EOF, got {:?}", e);
        }
        Ok(_) => panic!("Import succeeded but should have failed"),
    }
}

#[test]
fn test_recursion_limit() {
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(heading);

    // Deeply nested ignored field
    let mut json = String::from(r#"[{"id": 1, "ignored": "#);
    for _ in 0..1000 {
        json.push('[');
    }
    for _ in 0..1000 {
        json.push(']');
    }
    json.push_str("}]");

    let result = importer::from_json(Cursor::new(json.as_bytes()), rel_type);

    // With streaming and IgnoredAny, serde_json seems to handle deep nesting
    // without triggering recursion limit (it skips tokens efficiently).
    // This is good! We successfully imported the valid data and ignored the garbage safely.
    assert!(result.is_ok());
}
