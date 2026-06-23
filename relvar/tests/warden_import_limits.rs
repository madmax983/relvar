use relvar::tools::*;
use relvar_core::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_json_import_limit_rows() {
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(heading);

    // Create JSON with 100,001 items (limit is 100,000)
    // We can simulate this with a smaller limit if we could configure it,
    // but constants are hardcoded.
    // So we generate a large string. It's about 1MB-2MB of JSON, which is fine for test.
    let mut json = String::with_capacity(2_000_000);
    json.push('[');
    for i in 0..100_001 {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!("{{\"id\": {}}}", i));
    }
    json.push(']');

    let result = from_json(json.as_bytes(), rel_type);

    assert!(result.is_err());
    match result.unwrap_err() {
        ImporterError::LimitExceeded(msg) => {
            assert!(msg.contains("Max rows"));
        }
        // It might be wrapped in JsonError?
        // No, I explicitly map it in from_json:
        // if e.to_string().contains("Size limit exceeded") -> LimitExceeded
        // But RelationVisitor returns serde::de::Error::custom("Size limit exceeded...")
        // serde_json wraps custom errors.
        e => panic!("Expected LimitExceeded, got {:?}", e),
    }
}

#[test]
fn test_csv_import_limit_rows() {
    let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type = RelationType::new(heading);

    // Create CSV with 100,001 rows
    let mut csv = String::with_capacity(1_000_000);
    csv.push_str("id\n");
    for i in 0..100_001 {
        csv.push_str(&format!("{}\n", i));
    }

    let result = from_csv(csv.as_bytes(), rel_type, ',');

    assert!(result.is_err());
    match result.unwrap_err() {
        ImporterError::LimitExceeded(msg) => {
            assert!(msg.contains("Max rows"));
        }
        e => panic!("Expected LimitExceeded, got {:?}", e),
    }
}

#[test]
fn test_csv_line_length_limit() {
    let heading = TupleType::new().with_attribute("data", ScalarType::String);
    let rel_type = RelationType::new(heading);

    // Create CSV with a very long line (1MB + 10 bytes)
    let mut csv = String::with_capacity(1_100_000);
    csv.push_str("data\n");
    // "long_string"
    csv.push('"');
    for _ in 0..1_000_005 {
        csv.push('a');
    }
    csv.push('"');
    csv.push('\n');

    let result = from_csv(csv.as_bytes(), rel_type, ',');

    assert!(result.is_err());
    match result.unwrap_err() {
        ImporterError::LimitExceeded(msg) => {
            assert!(msg.contains("CSV Line too long"));
        }
        e => panic!("Expected LimitExceeded, got {:?}", e),
    }
}

#[test]
fn test_csv_line_length_limit_no_newline() {
    let heading = TupleType::new().with_attribute("data", ScalarType::String);
    let rel_type = RelationType::new(heading);

    // Create CSV with a very long line without newline at end
    let mut csv = String::with_capacity(1_100_000);
    csv.push_str("data\n");
    csv.push('"');
    for _ in 0..1_000_005 {
        csv.push('a');
    }
    csv.push('"');
    // No newline

    let result = from_csv(csv.as_bytes(), rel_type, ',');

    assert!(result.is_err());
    match result.unwrap_err() {
        ImporterError::LimitExceeded(msg) => {
            assert!(msg.contains("CSV Line too long"));
        }
        e => panic!("Expected LimitExceeded, got {:?}", e),
    }
}
