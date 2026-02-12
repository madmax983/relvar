//! Exporter module for Relvar.
//!
//! This module provides functionality to export relations to various formats,
//! making it easy to integrate with other tools or visualize data.
//!
//! # Supported Formats
//!
//! - **CSV**: Comma-Separated Values, compatible with Excel, Google Sheets, etc.
//! - **JSON**: JavaScript Object Notation, for web APIs and data interchange.
//! - **ASCII Table**: Human-readable text table, great for CLI output.
//!
//! # Deterministic Output
//!
//! Relvar relations are unordered sets, but exports from this module are
//! **deterministic**. Tuples are sorted by their values before export, ensuring
//! that the same relation always produces the exact same output.
//!
//! # Examples
//!
//! ```
//! use relvar_core::values::Relation;
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use relvar_core::tuple;
//! use relvar::experimental::exporter;
//!
//! // Create a sample relation
//! let heading = TupleType::new()
//!     .with_attribute("id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//! let mut relation = Relation::new(RelationType::new(heading));
//!
//! relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
//! relation.insert(tuple! { id: 2i64, name: "Bob" }).unwrap();
//!
//! // Export to CSV
//! let csv = exporter::to_csv(&relation, ',').unwrap();
//! assert!(csv.contains("id,name"));
//! assert!(csv.contains("1,\"Alice\""));
//!
//! // Export to JSON
//! let json = exporter::to_json(&relation).unwrap();
//! assert!(json.contains("\"name\": \"Alice\""));
//!
//! // Export to ASCII Table
//! let table = exporter::to_ascii_table(&relation);
//! println!("{}", table);
//! ```

use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::cmp::Ordering;
use thiserror::Error;

/// Errors that can occur during export.
#[derive(Debug, Error)]
pub enum ExporterError {
    /// JSON serialization error.
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// formatting error
    #[error("Formatting error: {0}")]
    FmtError(#[from] std::fmt::Error),
}

/// A wrapper around a tuple to allow sorting.
///
/// Tuples in Relvar are unordered sets, but for deterministic export
/// we need a consistent ordering.
#[derive(Debug, PartialEq, Eq)]
struct SortableTuple<'a>(&'a Tuple);

impl<'a> PartialOrd for SortableTuple<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for SortableTuple<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Tuples are compared by their values.
        // Since Tuple stores values in a BTreeMap, iterating values()
        // yields them in key-sorted order, which is perfect for deterministic comparison.
        self.0.values().cmp(other.0.values())
    }
}

/// Exports the relation to a CSV string.
///
/// The output includes a header row with attribute names, followed by
/// one row per tuple. String values are quoted, and quotes within strings
/// are escaped by doubling them (standard CSV escaping).
///
/// # Arguments
///
/// * `relation` - The relation to export.
/// * `delimiter` - The character to use as a delimiter (e.g., `,` for CSV, `\t` for TSV).
///
/// # Returns
///
/// A `String` containing the CSV data, or an `ExporterError` if formatting fails.
///
/// # Example
///
/// ```
/// use relvar_core::values::Relation;
/// use relvar_core::types::{RelationType, TupleType, ScalarType};
/// use relvar_core::tuple;
/// use relvar::experimental::exporter::to_csv;
///
/// let heading = TupleType::new()
///     .with_attribute("col1", ScalarType::Int)
///     .with_attribute("col2", ScalarType::String);
/// let mut relation = Relation::new(RelationType::new(heading));
/// relation.insert(tuple! { col1: 1i64, col2: "foo" }).unwrap();
///
/// let csv = to_csv(&relation, ',').unwrap();
/// assert_eq!(csv, "col1,col2\n1,\"foo\"\n");
/// ```
pub fn to_csv(relation: &Relation, delimiter: char) -> Result<String, ExporterError> {
    let headers: Vec<&String> = relation
        .relation_type()
        .heading()
        .attribute_names()
        .collect();
    let mut output = String::new();

    // Write headers
    for (i, header) in headers.iter().enumerate() {
        if i > 0 {
            output.push(delimiter);
        }
        output.push_str(header);
    }
    output.push('\n');

    // Sort tuples
    let mut tuples: Vec<SortableTuple> = relation.tuples().map(SortableTuple).collect();
    tuples.sort();

    // Write rows
    for tuple in tuples {
        for (i, header) in headers.iter().enumerate() {
            if i > 0 {
                output.push(delimiter);
            }
            if let Some(val) = tuple.0.get(header) {
                output.push_str(&format_scalar_csv(val));
            }
        }
        output.push('\n');
    }

    Ok(output)
}

/// Exports the relation to a JSON string.
///
/// The result is a JSON array of objects, where each object represents a tuple.
/// The keys are attribute names, and values are the corresponding scalar values.
///
/// Note: Scalar values are flattened to standard JSON types where possible:
/// - `Int`, `Float` -> JSON Number
/// - `String` -> JSON String
/// - `Bool` -> JSON Boolean
/// - `Bytes` -> JSON Array of numbers
/// - `Relation`, `UserDefined` -> String representation (simplification)
///
/// # Arguments
///
/// * `relation` - The relation to export.
///
/// # Returns
///
/// A pretty-printed JSON string.
///
/// # Example
///
/// ```
/// use relvar_core::values::Relation;
/// use relvar_core::types::{RelationType, TupleType, ScalarType};
/// use relvar_core::tuple;
/// use relvar::experimental::exporter::to_json;
///
/// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
/// let mut relation = Relation::new(RelationType::new(heading));
/// relation.insert(tuple! { id: 1i64 }).unwrap();
///
/// let json = to_json(&relation).unwrap();
/// // [
/// //   {
/// //     "id": 1
/// //   }
/// // ]
/// assert!(json.contains("\"id\": 1"));
/// ```
pub fn to_json(relation: &Relation) -> Result<String, ExporterError> {
    // Sort tuples first
    let mut tuples: Vec<SortableTuple> = relation.tuples().map(SortableTuple).collect();
    tuples.sort();

    // Convert tuples to simplified JSON objects
    let mut json_objects = Vec::with_capacity(tuples.len());

    for t in tuples {
        let mut map = serde_json::Map::new();
        // Use explicit iteration to avoid ambiguity and help static analysis
        // Note: t.0.values() returns &BTreeMap<String, ScalarValue>
        for (key, val) in t.0.values() {
            map.insert(key.clone(), scalar_to_json(val));
        }
        json_objects.push(map);
    }

    serde_json::to_string_pretty(&json_objects).map_err(ExporterError::JsonError)
}

/// Helper function to convert Relvar ScalarValue to serde_json::Value.
///
/// Flattens type wrappers to standard JSON types.
fn scalar_to_json(val: &ScalarValue) -> serde_json::Value {
    match val {
        ScalarValue::Int(v) => serde_json::Value::Number((*v).into()),
        ScalarValue::Float(v) => serde_json::Number::from_f64(*v)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        ScalarValue::String(v) => serde_json::Value::String(v.clone()),
        ScalarValue::Bool(v) => serde_json::Value::Bool(*v),
        ScalarValue::Bytes(v) => serde_json::Value::Array(
            v.iter()
                .map(|b| serde_json::Value::Number((*b).into()))
                .collect(),
        ),
        ScalarValue::Relation(_) => serde_json::Value::String("<Relation>".to_string()),
        ScalarValue::UserDefined { .. } => serde_json::Value::String("<UserDefined>".to_string()),
    }
}

/// Exports the relation to a formatted ASCII table.
///
/// This format is designed for human readability in terminal output.
/// It automatically calculates column widths to fit the content.
///
/// # Arguments
///
/// * `relation` - The relation to export.
///
/// # Returns
///
/// A string containing the ASCII table.
///
/// # Example
///
/// ```
/// use relvar_core::values::Relation;
/// use relvar_core::types::{RelationType, TupleType, ScalarType};
/// use relvar_core::tuple;
/// use relvar::experimental::exporter::to_ascii_table;
///
/// let heading = TupleType::new()
///     .with_attribute("name", ScalarType::String)
///     .with_attribute("score", ScalarType::Int);
/// let mut relation = Relation::new(RelationType::new(heading));
/// relation.insert(tuple! { name: "Alice", score: 100i64 }).unwrap();
///
/// let table = to_ascii_table(&relation);
/// println!("{}", table);
/// // +-------+-------+
/// // | name  | score |
/// // +-------+-------+
/// // | Alice | 100   |
/// // +-------+-------+
/// ```
pub fn to_ascii_table(relation: &Relation) -> String {
    let headers: Vec<&String> = relation
        .relation_type()
        .heading()
        .attribute_names()
        .collect();
    if headers.is_empty() {
        return String::from("(empty relation)");
    }

    // Sort tuples
    let mut tuples: Vec<SortableTuple> = relation.tuples().map(SortableTuple).collect();
    tuples.sort();

    // Calculate column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();

    // Pass 1: Measure data widths
    let rows: Vec<Vec<String>> = tuples
        .iter()
        .map(|t| {
            headers
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    let s = if let Some(val) = t.0.get(h) {
                        format_scalar_table(val)
                    } else {
                        String::new()
                    };
                    if s.len() > widths[i] {
                        widths[i] = s.len();
                    }
                    s
                })
                .collect()
        })
        .collect();

    let mut output = String::new();

    // Helper to draw separator line
    let draw_sep = |out: &mut String, widths: &[usize]| {
        out.push('+');
        for w in widths {
            out.push('-');
            out.push_str(&"-".repeat(*w));
            out.push('-');
            out.push('+');
        }
        out.push('\n');
    };

    // Top border
    draw_sep(&mut output, &widths);

    // Header row
    output.push('|');
    for (i, header) in headers.iter().enumerate() {
        output.push(' ');
        output.push_str(header);
        output.push_str(&" ".repeat(widths[i] - header.len()));
        output.push(' ');
        output.push('|');
    }
    output.push('\n');

    // Header separator
    draw_sep(&mut output, &widths);

    // Data rows
    for row in rows {
        output.push('|');
        for (i, cell) in row.iter().enumerate() {
            output.push(' ');
            output.push_str(cell);
            output.push_str(&" ".repeat(widths[i] - cell.len()));
            output.push(' ');
            output.push('|');
        }
        output.push('\n');
    }

    // Bottom border
    draw_sep(&mut output, &widths);

    output
}

fn format_scalar_csv(val: &ScalarValue) -> String {
    match val {
        ScalarValue::Int(v) => v.to_string(),
        ScalarValue::Float(v) => v.to_string(),
        ScalarValue::String(v) => format!("\"{}\"", v.replace("\"", "\"\"")), // CSV escaping
        ScalarValue::Bool(v) => v.to_string(),
        ScalarValue::Bytes(v) => format!("{:?}", v),
        ScalarValue::Relation(_) => "<Relation>".to_string(),
        ScalarValue::UserDefined { .. } => "<UserDefined>".to_string(),
    }
}

fn format_scalar_table(val: &ScalarValue) -> String {
    match val {
        ScalarValue::Int(v) => v.to_string(),
        ScalarValue::Float(v) => format!("{:.2}", v), // Format floats nicer for table
        ScalarValue::String(v) => v.clone(),
        ScalarValue::Bool(v) => v.to_string(),
        ScalarValue::Bytes(_) => "<Bytes>".to_string(),
        ScalarValue::Relation(_) => "<Relation>".to_string(),
        ScalarValue::UserDefined { .. } => "<UserDefined>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    fn create_test_relation() -> Relation {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("active", ScalarType::Bool);

        let mut relation = Relation::new(RelationType::new(heading));

        relation
            .insert(tuple! { id: 1i64, name: "Alice", active: true })
            .unwrap();
        relation
            .insert(tuple! { id: 2i64, name: "Bob", active: false })
            .unwrap();

        relation
    }

    #[test]
    fn test_to_csv() {
        let relation = create_test_relation();

        let csv = to_csv(&relation, ',').unwrap();

        // Expected output (sorted by tuple content)
        // Note: The order of columns in CSV depends on the iteration order of attribute_names()
        // which comes from BTreeMap keys (alphabetical).
        // TupleType stores attributes in BTreeMap, so keys are sorted.
        // Columns: active, id, name
        let expected = "active,id,name\nfalse,2,\"Bob\"\ntrue,1,\"Alice\"\n";

        assert_eq!(csv, expected);
    }

    #[test]
    fn test_to_json() {
        let relation = create_test_relation();

        let json = to_json(&relation).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(val.is_array());
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_to_ascii_table() {
        let relation = create_test_relation();

        let table = to_ascii_table(&relation);

        // Bob comes first (false < true)
        // active | id | name
        // false  | 2  | Bob
        // true   | 1  | Alice

        println!("{}", table);

        assert!(table.contains("| active | id | name  |"));
        assert!(table.contains("| false  | 2  | Bob   |"));
        assert!(table.contains("| true   | 1  | Alice |"));
    }

    #[test]
    fn test_json_export_all_types() {
        use relvar_core::values::ScalarValue;

        // Create a nested relation for the Relation type test
        let nested_heading = TupleType::new().with_attribute("x", ScalarType::Int);
        let mut nested_rel = Relation::new(RelationType::new(nested_heading.clone()));
        nested_rel.insert(tuple! { x: 10i64 }).unwrap();

        // Create a user defined type
        let user_type = ScalarType::user_defined("MyInt", ScalarType::Int);
        let user_val = user_type.selector(ScalarValue::Int(42)).unwrap();

        let heading = TupleType::new()
            .with_attribute("int_col", ScalarType::Int)
            .with_attribute("float_col", ScalarType::Float)
            .with_attribute("str_col", ScalarType::String)
            .with_attribute("bool_col", ScalarType::Bool)
            .with_attribute("bytes_col", ScalarType::Bytes)
            .with_attribute(
                "rel_col",
                ScalarType::Relation(Box::new(RelationType::new(nested_heading))),
            )
            .with_attribute("user_col", user_type);

        let mut relation = Relation::new(RelationType::new(heading));

        // Note: Using HashMap directly because the tuple! macro doesn't support all complex types easily inline
        use std::collections::HashMap;
        let mut values = HashMap::new();
        values.insert("int_col".to_string(), ScalarValue::Int(123));
        values.insert("float_col".to_string(), ScalarValue::Float(12.34));
        values.insert(
            "str_col".to_string(),
            ScalarValue::String("hello".to_string()),
        );
        values.insert("bool_col".to_string(), ScalarValue::Bool(true));
        values.insert("bytes_col".to_string(), ScalarValue::Bytes(vec![1, 2, 3]));
        values.insert("rel_col".to_string(), ScalarValue::Relation(nested_rel));
        values.insert("user_col".to_string(), user_val);

        let tuple = Tuple::new(relation.relation_type().heading().clone(), values).unwrap();
        relation.insert(tuple).unwrap();

        let json = to_json(&relation).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = &parsed[0];

        assert_eq!(obj["int_col"], 123);
        assert_eq!(obj["float_col"], 12.34);
        assert_eq!(obj["str_col"], "hello");
        assert_eq!(obj["bool_col"], true);
        assert_eq!(obj["bytes_col"], serde_json::json!([1, 2, 3]));
        assert_eq!(obj["rel_col"], "<Relation>");
        assert_eq!(obj["user_col"], "<UserDefined>");
    }
}
