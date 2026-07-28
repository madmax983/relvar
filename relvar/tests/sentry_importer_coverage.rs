use relvar::data::importer::{ImporterError, from_csv, from_json};
use relvar_core::types::{RelationType, ScalarType, TupleType};

#[test]
fn test_sentry_importer_empty_csv() {
    let heading = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
    let rel_type = RelationType::new(heading);

    let reader = std::io::Cursor::new(b"");
    let result = from_csv(reader, rel_type.clone(), ',');
    assert!(matches!(result, Err(ImporterError::FormatError(ref e)) if e == "Empty CSV input"));
}

#[test]
fn test_sentry_importer_empty_lines_csv() {
    let heading = TupleType::new().with_attribute("id".to_string(), ScalarType::Int);
    let rel_type = RelationType::new(heading);

    let reader = std::io::Cursor::new(b"id\n\n\n\n1\n");
    let result = from_csv(reader, rel_type.clone(), ',').unwrap();
    assert_eq!(result.cardinality(), 1);
}

#[test]
fn test_sentry_importer_json_errors() {
    let heading = TupleType::new().with_attribute("val".to_string(), ScalarType::Int);
    let rel_type = RelationType::new(heading);

    let reader = std::io::Cursor::new(b"[{\"val\": true}]");
    let result = from_json(reader, rel_type.clone());
    assert!(matches!(result, Err(ImporterError::JsonError(_))));

    let max_u64_str = format!("[{{\"val\": {}}}]", u64::MAX);
    let reader = std::io::Cursor::new(max_u64_str.as_bytes());
    let result = from_json(reader, rel_type.clone());
    assert!(matches!(result, Err(ImporterError::JsonError(_))));

    let heading_bool = TupleType::new().with_attribute("val".to_string(), ScalarType::Bool);
    let rel_type_bool = RelationType::new(heading_bool);
    let reader = std::io::Cursor::new(b"[{\"val\": true}]");
    let result = from_json(reader, rel_type_bool).unwrap();
    assert_eq!(result.cardinality(), 1);

    let heading_float = TupleType::new().with_attribute("val".to_string(), ScalarType::Float);
    let rel_type_float = RelationType::new(heading_float);
    let reader = std::io::Cursor::new(b"[{\"val\": 123}]");
    let result = from_json(reader, rel_type_float.clone()).unwrap();
    assert_eq!(result.cardinality(), 1);

    let reader = std::io::Cursor::new(b"[{\"val\": 1.5}]");
    let result = from_json(reader, rel_type.clone());
    assert!(matches!(result, Err(ImporterError::JsonError(_))));

    let reader = std::io::Cursor::new(b"[{\"val\": \"1.5\"}]");
    let result = from_json(reader, rel_type_float.clone());
    assert!(matches!(result, Err(ImporterError::JsonError(_))));

    let heading_string = TupleType::new().with_attribute("val".to_string(), ScalarType::String);
    let rel_type_string = RelationType::new(heading_string.clone());
    let reader = std::io::Cursor::new(b"[{\"val\": -42}]");
    let result = from_json(reader, rel_type_string.clone());
    assert!(matches!(result, Err(ImporterError::JsonError(_))));

    let reader = std::io::Cursor::new(b"[{\"val\": 1.5}]");
    let result = from_json(reader, rel_type_string.clone());
    assert!(matches!(result, Err(ImporterError::JsonError(_))));

    let reader = std::io::Cursor::new(b"[{\"val\": \"42\"}]");
    let result = from_json(reader, rel_type.clone());
    assert!(matches!(result, Err(ImporterError::JsonError(_))));
}

#[test]
fn test_sentry_csv_parse_types() {
    let heading = TupleType::new().with_attribute("b".to_string(), ScalarType::Bool);
    let rel_type = RelationType::new(heading);
    let reader = std::io::Cursor::new(b"b\ninvalid_bool");
    let result = from_csv(reader, rel_type.clone(), ',');
    assert!(matches!(result, Err(ImporterError::TypeError(_, _, _))));

    let heading = TupleType::new().with_attribute("f".to_string(), ScalarType::Float);
    let rel_type = RelationType::new(heading);
    let reader = std::io::Cursor::new(b"f\ninvalid_float");
    let result = from_csv(reader, rel_type.clone(), ',');
    assert!(matches!(result, Err(ImporterError::TypeError(_, _, _))));

    let heading = TupleType::new().with_attribute("by".to_string(), ScalarType::Bytes);
    let rel_type = RelationType::new(heading);
    let reader = std::io::Cursor::new(b"by\ninvalid_bytes");
    let result = from_csv(reader, rel_type.clone(), ',');
    assert!(matches!(result, Err(ImporterError::TypeError(_, _, _))));
}

#[test]
fn test_sentry_json_seq() {
    let sub_heading = TupleType::new().with_attribute("inner".to_string(), ScalarType::Int);
    let sub_rel_type = RelationType::new(sub_heading);
    let heading = TupleType::new().with_attribute(
        "val".to_string(),
        ScalarType::Relation(Box::new(sub_rel_type)),
    );
    let rel_type = RelationType::new(heading);

    let reader = std::io::Cursor::new(b"[{\"val\": [{\"inner\": 42}]}]");
    let result = from_json(reader, rel_type.clone()).unwrap();
    assert_eq!(result.cardinality(), 1);

    let reader = std::io::Cursor::new(b"[{\"val\": [1, 2]}]");
    let result = from_json(reader, rel_type.clone());
    assert!(matches!(result, Err(ImporterError::JsonError(_))));
}

#[test]
fn test_sentry_csv_global_limit() {
    let heading = TupleType::new().with_attribute("val".to_string(), ScalarType::String);
    let rel_type = RelationType::new(heading);

    struct InfinitySpacesReader {
        read_so_far: usize,
    }
    impl std::io::Read for InfinitySpacesReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            for b in buf.iter_mut() {
                *b = b' ';
            }
            self.read_so_far += buf.len();
            Ok(buf.len())
        }
    }

    let reader = InfinitySpacesReader { read_so_far: 0 };
    let result = from_csv(reader, rel_type.clone(), ',');
    assert!(matches!(result, Err(ImporterError::LimitExceeded(_))));
}

#[test]
fn test_sentry_json_string_limit() {
    let heading = TupleType::new().with_attribute("val".to_string(), ScalarType::String);
    let rel_type = RelationType::new(heading);

    // Create a string that exceeds 1_000_000 bytes
    let huge_string = "a".repeat(1_000_001);
    let huge_json = format!("[{{\"val\": \"{}\"}}]", huge_string);
    let reader = std::io::Cursor::new(huge_json.as_bytes());
    let result = from_json(reader, rel_type.clone());

    // Limits inside JSON scalar custom visitor return custom errors wrapped in JsonError
    assert!(matches!(
        result,
        Err(ImporterError::JsonError(_)) | Err(ImporterError::LimitExceeded(_))
    ));
}
