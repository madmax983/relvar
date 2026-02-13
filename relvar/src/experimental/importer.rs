//! Importer module for Relvar.
//!
//! This module provides functionality to import relations from various formats,
//! mirroring the exporter functionality. It strictly validates data against
//! a provided [`RelationType`].
//!
//! # Supported Formats
//!
//! - **JSON**: Imports from a JSON array of objects.
//! - **CSV**: Imports from Comma-Separated Values (with headers).
//!
//! # Type Coercion
//!
//! The importer attempts to coerce values to the expected type defined in the
//! `RelationType`. For example, a JSON number `1` can be imported as a `Float`
//! if the schema expects a `Float`.
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use relvar::experimental::importer;
//!
//! let heading = TupleType::new()
//!     .with_attribute("id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//! let rel_type = RelationType::new(heading);
//!
//! let json_data = r#"[
//!     {"id": 1, "name": "Alice"},
//!     {"id": 2, "name": "Bob"}
//! ]"#;
//!
//! let relation = importer::from_json(json_data.as_bytes(), rel_type).unwrap();
//! assert_eq!(relation.cardinality(), 2);
//! ```

use relvar_core::types::{RelationType, ScalarType};
use relvar_core::values::{Relation, ScalarValue, Tuple};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::io::BufRead;
use thiserror::Error;

/// Errors that can occur during import.
#[derive(Debug, Error)]
pub enum ImporterError {
    /// I/O error reading the input.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON parsing error.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Format error (e.g., malformed CSV line).
    #[error("Format error: {0}")]
    FormatError(String),

    /// Type mismatch or conversion error.
    #[error("Type error for attribute '{0}': expected {1}, got {2}")]
    TypeError(String, String, String),

    /// Missing value for a required attribute.
    #[error("Missing value for attribute '{0}'")]
    MissingValue(String),

    /// Relvar core error (e.g., duplicate tuple).
    #[error("Relvar error: {0}")]
    RelvarError(String),
}

/// Imports a relation from a JSON source.
///
/// Expects a JSON array of objects. Each object represents a tuple.
/// Keys in the objects must match attribute names in the `relation_type`.
///
/// # Arguments
///
/// * `reader` - Source of JSON data (e.g., file, string bytes).
/// * `relation_type` - The schema definition for the resulting relation.
pub fn from_json<R: std::io::Read>(
    reader: R,
    relation_type: RelationType,
) -> Result<Relation, ImporterError> {
    let root: JsonValue = serde_json::from_reader(reader)?;

    let array = root
        .as_array()
        .ok_or_else(|| ImporterError::FormatError("JSON root must be an array".to_string()))?;

    let mut relation = Relation::new(relation_type.clone());
    let heading = relation_type.heading();

    for (idx, item) in array.iter().enumerate() {
        let obj = item.as_object().ok_or_else(|| {
            ImporterError::FormatError(format!("Item at index {} is not an object", idx))
        })?;

        let mut values = BTreeMap::new();

        for attr_name in heading.attribute_names() {
            let attr_type = heading.get_attribute_type(attr_name).unwrap();

            if let Some(json_val) = obj.get(attr_name) {
                let scalar_val = json_value_to_scalar(json_val, attr_type).map_err(|e| {
                    ImporterError::TypeError(
                        attr_name.clone(),
                        format!("{:?}", attr_type),
                        e.to_string(),
                    )
                })?;
                values.insert(attr_name.clone(), scalar_val);
            } else {
                return Err(ImporterError::MissingValue(attr_name.clone()));
            }
        }

        let tuple = Tuple::new(heading.clone(), values)
            .map_err(|e| ImporterError::RelvarError(e.to_string()))?;

        // We ignore duplicate tuples as relations are sets, but we could warn or error.
        // For now, we just insert and ignore the result (Ok/Err).
        // Actually, insert returns Result, we should propagate error if it's not a duplicate error?
        // insert returns Result<bool, DatabaseError>. If it returns Ok(false), it was a duplicate.
        // If it returns Err, it's a constraint violation.
        // Relation::insert returns Result<bool, TupleError> or similar.
        // Let's check relation.rs, but typically it returns Result.
        let _ = relation
            .insert(tuple)
            .map_err(|e| ImporterError::RelvarError(e.to_string()))?;
    }

    Ok(relation)
}

fn json_value_to_scalar(
    value: &JsonValue,
    expected_type: &ScalarType,
) -> Result<ScalarValue, String> {
    match (value, expected_type) {
        // Int
        (JsonValue::Number(n), ScalarType::Int) => {
            if let Some(i) = n.as_i64() {
                Ok(ScalarValue::Int(i))
            } else {
                Err(format!("Value {} is not a valid Int", n))
            }
        }
        // Float
        (JsonValue::Number(n), ScalarType::Float) => {
            if let Some(f) = n.as_f64() {
                Ok(ScalarValue::Float(f))
            } else {
                // This case is rare as as_f64 usually works for numbers
                Err(format!("Value {} is not a valid Float", n))
            }
        }
        // String
        (JsonValue::String(s), ScalarType::String) => Ok(ScalarValue::String(s.clone())),
        // Bool
        (JsonValue::Bool(b), ScalarType::Bool) => Ok(ScalarValue::Bool(*b)),
        // Bytes (expect array of numbers)
        (JsonValue::Array(arr), ScalarType::Bytes) => {
            let mut bytes = Vec::with_capacity(arr.len());
            for v in arr {
                if let Some(n) = v.as_u64() {
                    if n <= 255 {
                        bytes.push(n as u8);
                    } else {
                        return Err(format!("Byte value {} out of range", n));
                    }
                } else {
                    return Err(format!("Invalid byte value {:?}", v));
                }
            }
            Ok(ScalarValue::Bytes(bytes))
        }
        // Relation (recursive)
        (JsonValue::Array(_), ScalarType::Relation(inner_type)) => {
            // We can reuse from_json logic here but we need a reader.
            // Convert current value back to string? Or refactor from_json to take Value.
            // Refactoring is cleaner but for now let's serialize to string.
            // This is inefficient but works for MVP.
            let inner_json = serde_json::to_string(value).map_err(|e| e.to_string())?;
            let rel =
                from_json(inner_json.as_bytes(), *inner_type.clone()).map_err(|e| e.to_string())?;
            Ok(ScalarValue::Relation(rel))
        }
        // UserDefined (recursive wrapper)
        (val, ScalarType::UserDefined { representation, .. }) => {
            let inner_val = json_value_to_scalar(val, representation)?;
            // We need to wrap it. But ScalarValue doesn't expose a raw constructor easily?
            // It has ScalarType::selector().
            expected_type
                .selector(inner_val)
                .map_err(|e| format!("Selector error: {:?}", e)) // generic debug error
        }
        _ => Err(format!(
            "Incompatible value {:?} for type {:?}",
            value, expected_type
        )),
    }
}

/// Imports a relation from a CSV source.
///
/// Expects a header row matching attribute names.
///
/// # Arguments
///
/// * `reader` - Source of CSV data.
/// * `relation_type` - The schema definition.
/// * `delimiter` - Field delimiter (e.g., `,`).
pub fn from_csv<R: std::io::Read>(
    reader: R,
    relation_type: RelationType,
    delimiter: char,
) -> Result<Relation, ImporterError> {
    let mut relation = Relation::new(relation_type.clone());
    let heading = relation_type.heading();
    let reader = std::io::BufReader::new(reader);
    let mut lines = reader.lines();

    // 1. Read Header
    let header_line = lines
        .next()
        .ok_or_else(|| ImporterError::FormatError("Empty CSV input".to_string()))??;

    let headers: Vec<String> = parse_csv_line(&header_line, delimiter);

    // Validate headers
    for attr_name in heading.attribute_names() {
        if !headers.contains(attr_name) {
            return Err(ImporterError::MissingValue(format!(
                "Header missing attribute '{}'",
                attr_name
            )));
        }
    }

    // 2. Read Rows
    for (line_idx, line) in lines.enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let fields = parse_csv_line(&line, delimiter);

        if fields.len() != headers.len() {
            return Err(ImporterError::FormatError(format!(
                "Row {} has {} fields, expected {}",
                line_idx + 1,
                fields.len(),
                headers.len()
            )));
        }

        let mut values = BTreeMap::new();

        for (i, field) in fields.iter().enumerate() {
            let attr_name = &headers[i];

            // If the schema has this attribute (we allow extra columns in CSV, but skip them if not in schema?)
            // For strictness, let's only import what's in schema.
            if let Some(attr_type) = heading.get_attribute_type(attr_name) {
                let scalar_val = str_to_scalar(field, attr_type).map_err(|e| {
                    ImporterError::TypeError(
                        attr_name.clone(),
                        format!("{:?}", attr_type),
                        e.to_string(),
                    )
                })?;
                values.insert(attr_name.clone(), scalar_val);
            }
        }

        let tuple = Tuple::new(heading.clone(), values)
            .map_err(|e| ImporterError::RelvarError(e.to_string()))?;

        let _ = relation
            .insert(tuple)
            .map_err(|e| ImporterError::RelvarError(e.to_string()))?;
    }

    Ok(relation)
}

/// Simple CSV line parser handling quotes.
fn parse_csv_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current_field = String::new();
    let mut inside_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if inside_quotes {
                    if let Some('"') = chars.peek() {
                        // Escaped quote
                        let _ = chars.next();
                        current_field.push('"');
                    } else {
                        // End of quote
                        inside_quotes = false;
                    }
                } else {
                    // Start of quote
                    inside_quotes = true;
                }
            }
            c if c == delimiter && !inside_quotes => {
                fields.push(current_field);
                current_field = String::new();
            }
            _ => {
                current_field.push(c);
            }
        }
    }
    fields.push(current_field);
    fields
}

fn str_to_scalar(s: &str, expected_type: &ScalarType) -> Result<ScalarValue, String> {
    match expected_type {
        ScalarType::Int => s
            .parse::<i64>()
            .map(ScalarValue::Int)
            .map_err(|e| e.to_string()),
        ScalarType::Float => s
            .parse::<f64>()
            .map(ScalarValue::Float)
            .map_err(|e| e.to_string()),
        ScalarType::String => Ok(ScalarValue::String(s.to_string())),
        ScalarType::Bool => s
            .parse::<bool>()
            .map(ScalarValue::Bool)
            .map_err(|e| e.to_string()),
        ScalarType::Bytes => {
            // Expect JSON array syntax in string? Or maybe hex?
            // For now, let's try to parse as JSON array of numbers
            let v: Vec<u8> =
                serde_json::from_str(s).map_err(|_| "Invalid bytes format".to_string())?;
            Ok(ScalarValue::Bytes(v))
        }
        ScalarType::Relation(_) => Err("Cannot import nested relations from CSV".to_string()),
        ScalarType::UserDefined { representation, .. } => {
            let inner_val = str_to_scalar(s, representation)?;
            expected_type
                .selector(inner_val)
                .map_err(|e| format!("{:?}", e))
        }
    }
}

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
        assert!(matches!(
            result.unwrap_err(),
            ImporterError::FormatError(msg) if msg.contains("JSON root must be an array")
        ));
    }

    #[test]
    fn test_json_error_item_not_object() {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let rel_type = RelationType::new(heading);
        let json = r#"[1, 2]"#; // Array of ints, not objects
        let result = from_json(json.as_bytes(), rel_type);
        assert!(matches!(
            result.unwrap_err(),
            ImporterError::FormatError(msg) if msg.contains("is not an object")
        ));
    }

    #[test]
    fn test_json_error_missing_value() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);
        let rel_type = RelationType::new(heading);
        let json = r#"[{"id": 1}]"#; // Missing "name"
        let result = from_json(json.as_bytes(), rel_type);
        assert!(matches!(
            result.unwrap_err(),
            ImporterError::MissingValue(attr) if attr == "name"
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
            ImporterError::TypeError(attr, _, _) if attr == "id"
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
        let json_invalid = r#"[{"data": [256]}]"#;
        let result_invalid = from_json(json_invalid.as_bytes(), rel_type.clone());
        assert!(matches!(
            result_invalid.unwrap_err(),
            ImporterError::TypeError(attr, _, _) if attr == "data"
        ));

        // Invalid byte type (string in array)
        let json_invalid_type = r#"[{"data": ["bad"]}]"#;
        let result_invalid_type = from_json(json_invalid_type.as_bytes(), rel_type);
        assert!(matches!(
            result_invalid_type.unwrap_err(),
            ImporterError::TypeError(attr, _, _) if attr == "data"
        ));
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
}
