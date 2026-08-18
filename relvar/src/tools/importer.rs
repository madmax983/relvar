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
//! use relvar::data::importer;
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

use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};
use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::io::BufRead;
use std::rc::Rc;
use thiserror::Error;

/// Errors that halt the import saga.
///
/// Importing data from the chaotic outside world into the structured realm
/// of a relational database is perilous. This enum categorizes the specific
/// tragedies that can occur, ranging from sheer physical exhaustion (I/O failures)
/// to philosophical disagreements (Type mismatches).
///
/// # Recovery
/// - **LimitExceeded:** The payload is too massive (e.g. >10MB for JSON). If this occurs, chunk your data into smaller files or streams.
/// - **TypeError:** Ensure your input data strictly adheres to the scalar types defined in your `RelationType` heading. The error specifies which attribute failed.
///
/// # Examples
///
/// ```
/// use relvar::tools::importer::ImporterError;
///
/// // Example of a DoS protection limit triggering.
/// // The user should be informed that their payload is too large, and they should chunk it.
/// let error = ImporterError::LimitExceeded("Input exceeds 10MB".to_string());
/// assert!(error.to_string().contains("Size limit exceeded"));
/// ```
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

    /// Size limit exceeded.
    #[error("Size limit exceeded: {0}")]
    LimitExceeded(String),
}

const MAX_STRING_LEN: usize = 1_000_000; // 1MB
const MAX_BYTES_LEN: usize = 1_000_000; // 1MB
const MAX_IMPORT_ROWS: usize = 100_000; // 100k rows
const MAX_CSV_LINE_LEN: usize = 1_000_000; // 1MB

/// Imports a relation from a JSON source.
///
/// Expects a JSON array of objects. Each object represents a tuple.
/// Keys in the objects must match attribute names in the `relation_type`.
///
/// # Arguments
///
/// * `reader` - Source of JSON data (e.g., file, string bytes).
/// * `relation_type` - The schema definition for the resulting relation.
///
/// # Examples
///
/// ```
/// use relvar::{TupleType, RelationType, ScalarType};
/// use relvar::tools::importer;
/// use std::io::Cursor;
///
/// let rel_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id".to_string(), ScalarType::Int)
///         .with_attribute("name".to_string(), ScalarType::String)
/// );
///
/// let json_data = r#"
/// [
///     {"id": 1, "name": "Alice"},
///     {"id": 2, "name": "Bob"}
/// ]
/// "#;
///
/// let relation = importer::from_json(Cursor::new(json_data), rel_type).unwrap();
/// assert_eq!(relation.cardinality(), 2);
/// ```
pub fn from_json<R: std::io::Read>(
    reader: R,
    relation_type: RelationType,
) -> Result<Relation, ImporterError> {
    // Security memory constraint: Using a Capped Reader. We limit the input stream to 10MB to
    // prevent serde_json from reading an unbounded malicious payload into memory.
    let mut capped_reader = reader.take(10_000_000);
    let mut deserializer = serde_json::Deserializer::from_reader(&mut capped_reader);
    let counter = Rc::new(RefCell::new(0usize));
    let seed = RelationSeed {
        relation_type,
        counter,
    };
    let relation = seed.deserialize(&mut deserializer).map_err(|e| {
        if e.to_string().contains("Size limit exceeded") {
            ImporterError::LimitExceeded(e.to_string())
        } else {
            ImporterError::JsonError(e)
        }
    })?;

    if capped_reader.limit() == 0 {
        return Err(ImporterError::LimitExceeded(
            "Global size limit exceeded: > 10MB".to_string(),
        ));
    }

    Ok(relation)
}

struct RelationSeed {
    relation_type: RelationType,
    counter: Rc<RefCell<usize>>,
}

impl<'de> DeserializeSeed<'de> for RelationSeed {
    type Value = Relation;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(RelationVisitor {
            relation_type: self.relation_type,
            counter: self.counter,
        })
    }
}

struct RelationVisitor {
    relation_type: RelationType,
    counter: Rc<RefCell<usize>>,
}

impl<'de> Visitor<'de> for RelationVisitor {
    type Value = Relation;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON array of objects")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut relation = Relation::new(self.relation_type.clone());
        let heading = self.relation_type.heading().clone();
        let tuple_seed = TupleSeed {
            heading,
            counter: self.counter.clone(),
        };

        while let Some(tuple) = seq.next_element_seed(tuple_seed.clone())? {
            let mut count = self.counter.borrow_mut();
            if *count >= MAX_IMPORT_ROWS {
                return Err(serde::de::Error::custom(format!(
                    "Size limit exceeded: Max rows: {}",
                    MAX_IMPORT_ROWS
                )));
            }
            *count += 1;
            drop(count);

            let _ = relation
                .insert(tuple)
                .map_err(|e| serde::de::Error::custom(format!("Relvar error: {}", e)))?;
        }

        Ok(relation)
    }
}

#[derive(Clone)]
struct TupleSeed {
    heading: TupleType,
    counter: Rc<RefCell<usize>>,
}

impl<'de> DeserializeSeed<'de> for TupleSeed {
    type Value = Tuple;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(TupleVisitor {
            heading: self.heading,
            counter: self.counter,
        })
    }
}

struct TupleVisitor {
    heading: TupleType,
    counter: Rc<RefCell<usize>>,
}

impl<'de> Visitor<'de> for TupleVisitor {
    type Value = Tuple;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON object representing a tuple")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = BTreeMap::new();
        let heading = self.heading;

        while let Some(key) = map.next_key::<String>()? {
            if let Some(attr_type) = heading.get_attribute_type(&key) {
                let seed = ScalarValueSeed {
                    scalar_type: attr_type.clone(),
                    counter: self.counter.clone(),
                };
                let val = map.next_value_seed(seed)?;
                values.insert(key, val);
            } else {
                // Ignore unknown fields
                let _ = map.next_value::<serde::de::IgnoredAny>()?;
            }
        }

        Tuple::new(heading, values).map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}

struct ScalarValueSeed {
    scalar_type: ScalarType,
    counter: Rc<RefCell<usize>>,
}

impl<'de> DeserializeSeed<'de> for ScalarValueSeed {
    type Value = ScalarValue;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        match &self.scalar_type {
            ScalarType::UserDefined { representation, .. } => {
                let inner_seed = ScalarValueSeed {
                    scalar_type: *representation.clone(),
                    counter: self.counter,
                };
                let inner_val = inner_seed.deserialize(deserializer)?;
                relvar_core::values::ScalarValue::select(&self.scalar_type, inner_val)
                    .map_err(|e| serde::de::Error::custom(format!("Selector error: {:?}", e)))
            }
            _ => deserializer.deserialize_any(ScalarValueVisitor {
                scalar_type: self.scalar_type,
                counter: self.counter,
            }),
        }
    }
}

struct ScalarValueVisitor {
    scalar_type: ScalarType,
    counter: Rc<RefCell<usize>>,
}

impl<'de> Visitor<'de> for ScalarValueVisitor {
    type Value = ScalarValue;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a value of type {:?}", self.scalar_type)
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if self.scalar_type == ScalarType::Bool {
            Ok(ScalarValue::Bool(v))
        } else {
            Err(E::invalid_type(serde::de::Unexpected::Bool(v), &self))
        }
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if self.scalar_type == ScalarType::Int {
            Ok(ScalarValue::Int(v))
        } else if self.scalar_type == ScalarType::Float {
            Ok(ScalarValue::Float(v as f64))
        } else {
            Err(E::invalid_type(serde::de::Unexpected::Signed(v), &self))
        }
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if self.scalar_type == ScalarType::Int {
            if v <= i64::MAX as u64 {
                Ok(ScalarValue::Int(v as i64))
            } else {
                Err(E::custom(format!("Value {} too large for Int", v)))
            }
        } else if self.scalar_type == ScalarType::Float {
            Ok(ScalarValue::Float(v as f64))
        } else {
            Err(E::invalid_type(serde::de::Unexpected::Unsigned(v), &self))
        }
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if self.scalar_type == ScalarType::Float {
            Ok(ScalarValue::Float(v))
        } else {
            Err(E::invalid_type(serde::de::Unexpected::Float(v), &self))
        }
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if self.scalar_type == ScalarType::String {
            if v.len() > MAX_STRING_LEN {
                return Err(E::custom(format!(
                    "Size limit exceeded: String too long: {} > {}",
                    v.len(),
                    MAX_STRING_LEN
                )));
            }
            Ok(ScalarValue::String(v.to_string()))
        } else {
            Err(E::invalid_type(serde::de::Unexpected::Str(v), &self))
        }
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&v)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        match &self.scalar_type {
            ScalarType::Bytes => {
                let mut bytes = Vec::new();
                while let Some(v) = seq.next_element::<u8>()? {
                    if bytes.len() >= MAX_BYTES_LEN {
                        return Err(serde::de::Error::custom(format!(
                            "Size limit exceeded: Bytes too long: > {}",
                            MAX_BYTES_LEN
                        )));
                    }
                    bytes.push(v);
                }
                Ok(ScalarValue::Bytes(bytes))
            }
            ScalarType::Relation(inner_type) => {
                // Relation is array of objects (tuples).
                let mut relation = Relation::new(*inner_type.clone()); // Need to deref Box
                let tuple_seed = TupleSeed {
                    heading: inner_type.heading().clone(),
                    counter: self.counter.clone(),
                };

                while let Some(tuple) = seq.next_element_seed(tuple_seed.clone())? {
                    let mut count = self.counter.borrow_mut();
                    if *count >= MAX_IMPORT_ROWS {
                        return Err(serde::de::Error::custom(format!(
                            "Size limit exceeded: Max rows: {}",
                            MAX_IMPORT_ROWS
                        )));
                    }
                    *count += 1;
                    drop(count);

                    relation
                        .insert(tuple)
                        .map_err(|e| serde::de::Error::custom(format!("Relvar error: {}", e)))?;
                }
                Ok(ScalarValue::Relation(relation))
            }
            _ => Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Seq,
                &self,
            )),
        }
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
///
/// # Examples
///
/// ```
/// use relvar::{TupleType, RelationType, ScalarType};
/// use relvar::tools::importer;
/// use std::io::Cursor;
///
/// let rel_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id".to_string(), ScalarType::Int)
///         .with_attribute("name".to_string(), ScalarType::String)
/// );
///
/// let csv_data = "id,name\n1,Alice\n2,Bob\n";
///
/// let relation = importer::from_csv(Cursor::new(csv_data), rel_type, ',').unwrap();
/// assert_eq!(relation.cardinality(), 2);
/// ```
pub fn from_csv<R: std::io::Read>(
    reader: R,
    relation_type: RelationType,
    delimiter: char,
) -> Result<Relation, ImporterError> {
    // Security memory constraint: Using a Capped Reader. We limit the input stream to 10MB to
    // prevent unbounded malicious CSV payloads into memory.
    let mut capped_reader = reader.take(10_000_000);
    let mut relation = Relation::new(relation_type.clone());
    let heading = relation_type.heading();
    let mut reader = std::io::BufReader::new(&mut capped_reader);

    // Helper for safe line reading
    fn read_line_safe<B: BufRead>(
        reader: &mut B,
        buf: &mut String,
    ) -> Result<usize, ImporterError> {
        buf.clear();
        // Use reader.take() directly on the mutable reference.
        // B implements BufRead, so &mut B implements BufRead + Read.
        // We use take() from the Read trait on the reference itself to avoid moving B.
        let mut taker = std::io::Read::take(&mut *reader, MAX_CSV_LINE_LEN as u64);
        let n = taker.read_line(buf)?;

        if n == 0 {
            return Ok(0);
        }

        if buf.ends_with('\n') {
            return Ok(n);
        }

        // Check if there is more data (meaning we hit the limit)
        let available = reader.fill_buf()?;
        if !available.is_empty() {
            return Err(ImporterError::LimitExceeded(format!(
                "CSV Line too long: > {}",
                MAX_CSV_LINE_LEN
            )));
        }

        Ok(n)
    }

    // 1. Read Header
    let mut header_line = String::new();
    if read_line_safe(&mut reader, &mut header_line)? == 0 {
        return Err(ImporterError::FormatError("Empty CSV input".to_string()));
    }
    // Trim newline for parsing
    let header_line_trimmed = header_line.trim_end();

    let headers: Vec<String> = parse_csv_line(header_line_trimmed, delimiter);

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
    let mut line_buf = String::new();
    let mut line_idx = 0;
    while read_line_safe(&mut reader, &mut line_buf)? > 0 {
        if line_idx >= MAX_IMPORT_ROWS {
            return Err(ImporterError::LimitExceeded(format!(
                "Size limit exceeded: Max rows: {}",
                MAX_IMPORT_ROWS
            )));
        }

        let line = line_buf.trim_end();
        if line.trim().is_empty() {
            continue;
        }

        let fields = parse_csv_line(line, delimiter);

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

        line_idx += 1;
    }

    if capped_reader.limit() == 0 {
        return Err(ImporterError::LimitExceeded(
            "Global size limit exceeded: > 10MB".to_string(),
        ));
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
            relvar_core::values::ScalarValue::select(expected_type, inner_val)
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
