#![allow(clippy::module_inception)]
use super::*;
#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::types::TupleType;

    #[test]
    fn test_from_json_simple() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);
        let rel_type = RelationType::new(heading);

        let json = r#"[
            {"id": 1, "name": "Alice"},
            {"id": 2, "name": "Bob"}
        ]"#;

        let relation = from_json(json.as_bytes(), rel_type).unwrap();
        assert_eq!(relation.cardinality(), 2);
    }

    #[test]
    fn test_from_json_nested() {
        // Define UserType
        let user_id_type = ScalarType::user_defined("UserId", ScalarType::Int);

        let heading = TupleType::new()
            .with_attribute("uid", user_id_type.clone())
            .with_attribute("score", ScalarType::Float);
        let rel_type = RelationType::new(heading);

        let json = r#"[
            {"uid": 100, "score": 99.5},
            {"uid": 101, "score": 88.0}
        ]"#;

        let relation = from_json(json.as_bytes(), rel_type).unwrap();
        assert_eq!(relation.cardinality(), 2);

        let tuple = relation.tuples().next().unwrap();
        let val = tuple.get("uid").unwrap();
        assert_eq!(val.scalar_type(), user_id_type);
    }

    #[test]
    fn test_from_csv_simple() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("active", ScalarType::Bool);
        let rel_type = RelationType::new(heading);

        let csv = "id,name,active\n1,\"Alice\",true\n2,Bob,false";

        let relation = from_csv(csv.as_bytes(), rel_type, ',').unwrap();
        assert_eq!(relation.cardinality(), 2);
    }

    #[test]
    fn test_parse_csv_line() {
        let line = "1,\"Alice, Bob\",3";
        let fields = parse_csv_line(line, ',');
        assert_eq!(fields, vec!["1", "Alice, Bob", "3"]);

        let line = "1,\"Alice \"\"The Great\"\"\",3";
        let fields = parse_csv_line(line, ',');
        assert_eq!(fields, vec!["1", "Alice \"The Great\"", "3"]);
    }

    #[test]
    fn test_json_error_root_not_array() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        let json = r#"{"id": 1}"#; // Object, not array
        let result = from_json(json.as_bytes(), rel_type);
        // Deserializer error for wrong type
        assert!(matches!(result.unwrap_err(), ImporterError::JsonError(_)));
    }

    #[test]
    fn test_json_error_item_not_object() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        let json = r#"[1, 2]"#; // Array of ints, not objects
        let result = from_json(json.as_bytes(), rel_type);
        // Error message might vary depending on where deserialize_map fails
        // When expecting a map, but getting int, it returns invalid type
        assert!(result.is_err());
    }

    #[test]
    fn test_json_error_missing_value() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);
        let rel_type = RelationType::new(heading);
        let json = r#"[{"id": 1}]"#; // Missing "name"
        let result = from_json(json.as_bytes(), rel_type);
        // Tuple::new checks for missing attributes
        assert!(matches!(
            result.unwrap_err(),
            ImporterError::JsonError(e) if e.to_string().contains("Missing value for attribute") || e.to_string().contains("Relvar error")
        ));
    }

    #[test]
    fn test_json_error_type_mismatch() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        let json = r#"[{"id": "one"}]"#; // String instead of Int
        let result = from_json(json.as_bytes(), rel_type);
        assert!(matches!(
            result.unwrap_err(),
            ImporterError::JsonError(e) if e.to_string().contains("invalid type")
        ));
    }

    #[test]
    fn test_json_bytes_handling() {
        let heading = TupleType::new().with_attribute("data", ScalarType::Bytes);
        let rel_type = RelationType::new(heading);

        // Valid bytes
        let json_valid = r#"[{"data": [1, 2, 255]}]"#;
        let result = from_json(json_valid.as_bytes(), rel_type.clone());
        assert!(result.is_ok());
        let rel = result.unwrap();
        let tuple = rel.tuples().next().unwrap();
        assert_eq!(
            tuple.get("data"),
            Some(&ScalarValue::Bytes(vec![1, 2, 255]))
        );

        // Invalid byte (out of range)
        // Note: For Bytes, we read as u8, so 256 would fail with "out of range" or invalid type for u8
        let json_invalid = r#"[{"data": [256]}]"#;
        let result_invalid = from_json(json_invalid.as_bytes(), rel_type.clone());
        assert!(result_invalid.is_err());

        // Invalid byte type (string in array)
        let json_invalid_type = r#"[{"data": ["bad"]}]"#;
        let result_invalid_type = from_json(json_invalid_type.as_bytes(), rel_type);
        assert!(result_invalid_type.is_err());
    }

    #[test]
    fn test_json_nested_relation() {
        let inner_heading = TupleType::new().with_attribute("val", ScalarType::Int);
        let inner_rel_type = RelationType::new(inner_heading);

        let outer_heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("items", ScalarType::Relation(Box::new(inner_rel_type)));
        let outer_rel_type = RelationType::new(outer_heading);

        let json = r#"[
            {
                "id": 1,
                "items": [{"val": 10}, {"val": 20}]
            }
        ]"#;

        let result = from_json(json.as_bytes(), outer_rel_type);
        assert!(result.is_ok());
        let rel = result.unwrap();
        let tuple = rel.tuples().next().unwrap();

        match tuple.get("items") {
            Some(ScalarValue::Relation(inner)) => {
                assert_eq!(inner.cardinality(), 2);
            }
            _ => panic!("Expected relation"),
        }
    }

    #[test]
    fn test_csv_error_header_missing() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        let csv = "name\nAlice"; // Header "name", expected "id"
        let result = from_csv(csv.as_bytes(), rel_type, ',');
        assert!(matches!(
            result.unwrap_err(),
            ImporterError::MissingValue(msg) if msg.contains("Header missing attribute 'id'")
        ));
    }

    #[test]
    fn test_csv_error_field_count() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);
        let rel_type = RelationType::new(heading);
        let csv = "id,name\n1"; // Missing name value
        let result = from_csv(csv.as_bytes(), rel_type, ',');
        assert!(matches!(
            result.unwrap_err(),
            ImporterError::FormatError(msg) if msg.contains("Row 1 has 1 fields, expected 2")
        ));
    }

    #[test]
    fn test_csv_error_type_mismatch() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        let csv = "id\nnot_an_int";
        let result = from_csv(csv.as_bytes(), rel_type, ',');
        assert!(matches!(
            result.unwrap_err(),
            ImporterError::TypeError(attr, _, _) if attr == "id"
        ));
    }

    #[test]
    fn test_csv_bytes_and_user_defined() {
        let user_type = ScalarType::user_defined("UserId", ScalarType::Int);
        let heading = TupleType::new()
            .with_attribute("uid", user_type.clone())
            .with_attribute("data", ScalarType::Bytes);
        let rel_type = RelationType::new(heading);

        // CSV uses JSON array syntax for bytes
        let csv = "uid,data\n100,\"[1, 2, 3]\"";

        let result = from_csv(csv.as_bytes(), rel_type, ',');
        assert!(result.is_ok());
        let rel = result.unwrap();
        let tuple = rel.tuples().next().unwrap();

        assert_eq!(tuple.get("uid").unwrap().scalar_type(), user_type);
        assert_eq!(tuple.get("data"), Some(&ScalarValue::Bytes(vec![1, 2, 3])));
    }

    #[test]
    fn test_csv_nested_relation_error() {
        let inner_heading = TupleType::new().with_attribute("x", ScalarType::Int);
        let heading = TupleType::new().with_attribute(
            "rel",
            ScalarType::Relation(Box::new(RelationType::new(inner_heading))),
        );
        let rel_type = RelationType::new(heading);

        let csv = "rel\n[]";
        let result = from_csv(csv.as_bytes(), rel_type, ',');
        assert!(matches!(
            result.unwrap_err(),
            ImporterError::TypeError(attr, _, _) if attr == "rel"
        ));
    }

    #[test]
    fn test_json_limit_exceeded() {
        use std::io::Read;

        struct InfiniteJsonReader {
            count: usize,
        }

        impl Read for InfiniteJsonReader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let mut i = 0;
                while i < buf.len() {
                    if self.count == 0 {
                        buf[i] = b'[';
                    } else if self.count > 10_000_000 {
                        buf[i] = b']';
                    } else {
                        let s = b"{\"id\": 1},";
                        let idx = (self.count - 1) % s.len();
                        buf[i] = s[idx];
                    }
                    self.count += 1;
                    i += 1;
                }
                Ok(buf.len())
            }
        }

        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        let reader = InfiniteJsonReader { count: 0 };

        let result = from_json(reader, rel_type);
        assert!(result.is_err());
        match result {
            Err(ImporterError::LimitExceeded(_msg)) => {
                // Return gracefully for any limit exceeded message.
            }
            _ => panic!("Expected LimitExceeded error, got {:?}", result),
        }
    }
}
