//! Type serialization for system catalog storage.
//!
//! This module provides string serialization for [`ScalarType`] instances to enable
//! storage in the system catalog. Type information must be persisted as strings
//! in catalog relations.
//!
//! # Format
//!
//! Types are serialized using a human-readable format:
//!
//! - Built-in types: `"Int"`, `"Float"`, `"String"`, `"Bool"`, `"Bytes"`
//! - Relation types: `"Relation(HEADING)"` where HEADING is serialized tuple type
//! - User-defined types: `"UserDefined(Name,RepType)"` where RepType is recursively serialized
//!
//! # Example
//!
//! ```
//! use relvar::types::ScalarType;
//! use relvar::storage::type_serializer::{serialize_scalar_type, deserialize_scalar_type};
//!
//! let int_type = ScalarType::Int;
//! let serialized = serialize_scalar_type(&int_type);
//! assert_eq!(serialized, "Int");
//!
//! let deserialized = deserialize_scalar_type(&serialized).unwrap();
//! assert_eq!(deserialized, int_type);
//! ```
//!
//! # TTM Compliance
//!
//! TTM RM Prescription 12: The system catalog is relational. Type information
//! must be stored as explicit values in catalog relvars, necessitating this
//! serialization layer.

use relvar_core::types::{RelationType, ScalarType, TupleType};
use thiserror::Error;

/// Errors that can occur during type serialization/deserialization.
#[derive(Debug, Error)]
pub enum TypeSerializerError {
    /// Failed to parse a type string.
    #[error("Invalid type format: {0}")]
    InvalidFormat(String),

    /// Encountered an unsupported type construct.
    #[error("Unsupported type: {0}")]
    Unsupported(String),
}

/// Serializes a [`ScalarType`] to a string representation.
///
/// The string format is designed to be human-readable and unambiguous.
/// All built-in types serialize to their variant name.
///
/// # Examples
///
/// ```
/// use relvar::types::ScalarType;
/// use relvar::storage::type_serializer::serialize_scalar_type;
///
/// assert_eq!(serialize_scalar_type(&ScalarType::Int), "Int");
/// assert_eq!(serialize_scalar_type(&ScalarType::String), "String");
///
/// let widget_id = ScalarType::user_defined("WidgetId", ScalarType::Int);
/// assert_eq!(serialize_scalar_type(&widget_id), "UserDefined(WidgetId,Int)");
/// ```
pub fn serialize_scalar_type(scalar_type: &ScalarType) -> String {
    match scalar_type {
        ScalarType::Int => "Int".to_string(),
        ScalarType::Float => "Float".to_string(),
        ScalarType::String => "String".to_string(),
        ScalarType::Bool => "Bool".to_string(),
        ScalarType::Bytes => "Bytes".to_string(),
        ScalarType::Relation(rel_type) => {
            // Serialize as Relation(attr1:Type1,attr2:Type2,...)
            let attrs = rel_type
                .heading()
                .attributes()
                .iter()
                .map(|(name, ty)| format!("{}:{}", name, serialize_scalar_type(ty)))
                .collect::<Vec<_>>()
                .join(",");
            format!("Relation({})", attrs)
        }
        ScalarType::UserDefined {
            name,
            representation,
        } => {
            format!(
                "UserDefined({},{})",
                name,
                serialize_scalar_type(representation)
            )
        }
    }
}

/// Deserializes a string to a [`ScalarType`].
///
/// This is the inverse of [`serialize_scalar_type`]. The input must be
/// a valid type string produced by serialization.
///
/// # Errors
///
/// Returns [`TypeSerializerError::InvalidFormat`] if the string cannot be parsed.
///
/// # Examples
///
/// ```
/// use relvar::types::ScalarType;
/// use relvar::storage::type_serializer::deserialize_scalar_type;
///
/// let int_type = deserialize_scalar_type("Int").unwrap();
/// assert_eq!(int_type, ScalarType::Int);
///
/// let result = deserialize_scalar_type("InvalidType");
/// assert!(result.is_err());
/// ```
pub fn deserialize_scalar_type(s: &str) -> Result<ScalarType, TypeSerializerError> {
    match s {
        "Int" => Ok(ScalarType::Int),
        "Float" => Ok(ScalarType::Float),
        "String" => Ok(ScalarType::String),
        "Bool" => Ok(ScalarType::Bool),
        "Bytes" => Ok(ScalarType::Bytes),
        _ if s.starts_with("Relation(") && s.ends_with(")") => {
            // Parse Relation(attr1:Type1,attr2:Type2,...)
            let inner = &s[9..s.len() - 1]; // Strip "Relation(" and ")"

            if inner.is_empty() {
                // Empty relation heading
                return Ok(ScalarType::Relation(Box::new(RelationType::new(
                    TupleType::new(),
                ))));
            }

            let mut tuple_type = TupleType::new();

            // Parse comma-separated attributes
            // Need to handle nested types with commas
            let attrs = parse_attributes(inner)?;

            for (attr_name, attr_type_str) in attrs {
                let attr_type = deserialize_scalar_type(&attr_type_str)?;
                tuple_type = tuple_type.with_attribute(attr_name, attr_type);
            }

            Ok(ScalarType::Relation(Box::new(RelationType::new(
                tuple_type,
            ))))
        }
        _ if s.starts_with("UserDefined(") && s.ends_with(")") => {
            // Parse UserDefined(Name,RepType)
            let inner = &s[12..s.len() - 1]; // Strip "UserDefined(" and ")"

            // Find the comma that separates name from representation
            // Need to handle nested types
            let comma_pos = find_top_level_comma(inner).ok_or_else(|| {
                TypeSerializerError::InvalidFormat(format!("Invalid UserDefined format: {}", s))
            })?;

            let name = inner[..comma_pos].to_string();
            let rep_type_str = &inner[comma_pos + 1..];
            let rep_type = deserialize_scalar_type(rep_type_str)?;

            Ok(ScalarType::user_defined(name, rep_type))
        }
        _ => Err(TypeSerializerError::InvalidFormat(format!(
            "Unknown type: {}",
            s
        ))),
    }
}

/// Finds the position of the top-level comma (not inside parentheses).
fn find_top_level_comma(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, ch) in s.chars().enumerate() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Parses attribute definitions from a string like "attr1:Type1,attr2:Type2".
/// Handles nested types with proper parenthesis matching.
fn parse_attributes(s: &str) -> Result<Vec<(String, String)>, TypeSerializerError> {
    if s.is_empty() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                // Found a top-level comma, parse the accumulated attribute
                let attr = parse_single_attribute(&current)?;
                result.push(attr);
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    // Don't forget the last attribute
    if !current.is_empty() {
        let attr = parse_single_attribute(&current)?;
        result.push(attr);
    }

    Ok(result)
}

/// Parses a single attribute definition like "attr1:Type1".
fn parse_single_attribute(s: &str) -> Result<(String, String), TypeSerializerError> {
    let colon_pos = s.find(':').ok_or_else(|| {
        TypeSerializerError::InvalidFormat(format!("Missing colon in attribute: {}", s))
    })?;

    let name = s[..colon_pos].trim().to_string();
    let type_str = s[colon_pos + 1..].trim().to_string();

    Ok((name, type_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Phase 1 tests: Built-in types
    #[test]
    fn test_serialize_int() {
        let type_def = ScalarType::Int;
        let serialized = serialize_scalar_type(&type_def);
        assert_eq!(serialized, "Int");
    }

    #[test]
    fn test_serialize_float() {
        let type_def = ScalarType::Float;
        let serialized = serialize_scalar_type(&type_def);
        assert_eq!(serialized, "Float");
    }

    #[test]
    fn test_serialize_string() {
        let type_def = ScalarType::String;
        let serialized = serialize_scalar_type(&type_def);
        assert_eq!(serialized, "String");
    }

    #[test]
    fn test_serialize_bool() {
        let type_def = ScalarType::Bool;
        let serialized = serialize_scalar_type(&type_def);
        assert_eq!(serialized, "Bool");
    }

    #[test]
    fn test_serialize_bytes() {
        let type_def = ScalarType::Bytes;
        let serialized = serialize_scalar_type(&type_def);
        assert_eq!(serialized, "Bytes");
    }

    #[test]
    fn test_serialize_relation() {
        // Create a relation type with a simple heading
        let tuple_type = TupleType::new()
            .with_attribute("id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String);
        let rel_type = RelationType::new(tuple_type);
        let type_def = ScalarType::Relation(Box::new(rel_type));

        let serialized = serialize_scalar_type(&type_def);
        // Relation types should include the heading information
        assert!(serialized.starts_with("Relation("));
        assert!(serialized.ends_with(")"));
        assert!(serialized.contains("id"));
        assert!(serialized.contains("name"));
    }

    #[test]
    fn test_serialize_user_defined() {
        let widget_id = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let serialized = serialize_scalar_type(&widget_id);
        assert_eq!(serialized, "UserDefined(WidgetId,Int)");
    }

    #[test]
    fn test_serialize_nested_user_defined() {
        // UserDefined types can have UserDefined representation
        let inner = ScalarType::user_defined("Inner", ScalarType::String);
        let outer = ScalarType::user_defined("Outer", inner);
        let serialized = serialize_scalar_type(&outer);
        assert_eq!(serialized, "UserDefined(Outer,UserDefined(Inner,String))");
    }

    // Phase 2 tests: Deserialization
    #[test]
    fn test_deserialize_int() {
        let result = deserialize_scalar_type("Int").unwrap();
        assert_eq!(result, ScalarType::Int);
    }

    #[test]
    fn test_deserialize_float() {
        let result = deserialize_scalar_type("Float").unwrap();
        assert_eq!(result, ScalarType::Float);
    }

    #[test]
    fn test_deserialize_string() {
        let result = deserialize_scalar_type("String").unwrap();
        assert_eq!(result, ScalarType::String);
    }

    #[test]
    fn test_deserialize_bool() {
        let result = deserialize_scalar_type("Bool").unwrap();
        assert_eq!(result, ScalarType::Bool);
    }

    #[test]
    fn test_deserialize_bytes() {
        let result = deserialize_scalar_type("Bytes").unwrap();
        assert_eq!(result, ScalarType::Bytes);
    }

    #[test]
    fn test_deserialize_user_defined() {
        let result = deserialize_scalar_type("UserDefined(WidgetId,Int)").unwrap();
        let expected = ScalarType::user_defined("WidgetId", ScalarType::Int);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_deserialize_nested_user_defined() {
        let result =
            deserialize_scalar_type("UserDefined(Outer,UserDefined(Inner,String))").unwrap();
        let inner = ScalarType::user_defined("Inner", ScalarType::String);
        let expected = ScalarType::user_defined("Outer", inner);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_deserialize_invalid_format() {
        let result = deserialize_scalar_type("NotAType");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TypeSerializerError::InvalidFormat(_)
        ));
    }

    #[test]
    fn test_deserialize_incomplete_user_defined() {
        let result = deserialize_scalar_type("UserDefined(WidgetId");
        assert!(result.is_err());
    }

    // Phase 3 tests: Round-trip verification
    #[test]
    fn test_roundtrip_all_types() {
        let types = vec![
            ScalarType::Int,
            ScalarType::Float,
            ScalarType::String,
            ScalarType::Bool,
            ScalarType::Bytes,
            ScalarType::user_defined("WidgetId", ScalarType::Int),
            ScalarType::user_defined("SupplierId", ScalarType::String),
        ];

        for type_def in types {
            let serialized = serialize_scalar_type(&type_def);
            let deserialized = deserialize_scalar_type(&serialized).unwrap();
            assert_eq!(
                type_def, deserialized,
                "Roundtrip failed for {:?}",
                type_def
            );
        }
    }

    #[test]
    fn test_roundtrip_relation_type() {
        let tuple_type = TupleType::new()
            .with_attribute("x".to_string(), ScalarType::Int)
            .with_attribute("y".to_string(), ScalarType::Float);
        let rel_type = RelationType::new(tuple_type);
        let type_def = ScalarType::Relation(Box::new(rel_type));

        let serialized = serialize_scalar_type(&type_def);
        let deserialized = deserialize_scalar_type(&serialized).unwrap();
        assert_eq!(type_def, deserialized);
    }
}
