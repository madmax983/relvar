//! Exporter module for Relvar.
//!
//! This module provides functionality to export relations to various formats
//! like CSV, JSON, and ASCII tables.
//!
//! # Example
//!
//! ```
//! use relvar::{tuple, experimental::exporter};
//! use relvar::types::{RelationType, TupleType, ScalarType};
//! use relvar::values::Relation;
//!
//! let rel_type = RelationType::new(
//!     TupleType::new()
//!         .with_attribute("id", ScalarType::Int)
//!         .with_attribute("name", ScalarType::String)
//! );
//! let mut relation = Relation::new(rel_type);
//! relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
//!
//! let csv = exporter::to_csv(&relation, ',').unwrap();
//! println!("{}", csv);
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
/// # Arguments
///
/// * `relation` - The relation to export.
/// * `delimiter` - The character to use as a delimiter (e.g., ',' or '\t').
///
/// # Example
///
/// ```
/// use relvar::{tuple, experimental::exporter};
/// use relvar::types::{RelationType, TupleType, ScalarType};
/// use relvar::values::Relation;
///
/// let rel_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id", ScalarType::Int)
///         .with_attribute("name", ScalarType::String)
/// );
/// let mut relation = Relation::new(rel_type);
/// relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
///
/// let csv = exporter::to_csv(&relation, ',').unwrap();
/// assert!(csv.contains("id,name"));
/// assert!(csv.contains("1,\"Alice\""));
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
/// Scalar values are mapped to their JSON equivalents (Int/Float -> Number,
/// String -> String, Bool -> Boolean).
///
/// # Example
///
/// ```
/// use relvar::{tuple, experimental::exporter};
/// use relvar::types::{RelationType, TupleType, ScalarType};
/// use relvar::values::Relation;
///
/// let rel_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id", ScalarType::Int)
///         .with_attribute("name", ScalarType::String)
/// );
/// let mut relation = Relation::new(rel_type);
/// relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
///
/// let json = exporter::to_json(&relation).unwrap();
/// // Output: [ { "id": 1, "name": "Alice" } ]
/// assert!(json.contains("\"id\": 1"));
/// assert!(json.contains("\"name\": \"Alice\""));
/// ```
pub fn to_json(relation: &Relation) -> Result<String, ExporterError> {
    // Sort tuples first
    let mut tuples: Vec<SortableTuple> = relation.tuples().map(SortableTuple).collect();
    tuples.sort();

    let mut json_rows = Vec::with_capacity(tuples.len());

    for tuple in tuples {
        let mut row = serde_json::Map::new();
        // Values in BTreeMap are already sorted by key (attribute name)
        for (key, val) in tuple.0.values() {
            row.insert(key.clone(), scalar_to_json(val));
        }
        json_rows.push(serde_json::Value::Object(row));
    }

    serde_json::to_string_pretty(&json_rows).map_err(ExporterError::JsonError)
}

fn scalar_to_json(val: &ScalarValue) -> serde_json::Value {
    match val {
        ScalarValue::Int(v) => serde_json::Value::Number((*v).into()),
        ScalarValue::Float(v) => {
            if let Some(n) = serde_json::Number::from_f64(*v) {
                serde_json::Value::Number(n)
            } else {
                serde_json::Value::Null // JSON doesn't support NaN/Infinity
            }
        }
        ScalarValue::String(v) => serde_json::Value::String(v.clone()),
        ScalarValue::Bool(v) => serde_json::Value::Bool(*v),
        ScalarValue::Bytes(v) => serde_json::Value::Array(
            v.iter()
                .map(|b| serde_json::Value::Number((*b).into()))
                .collect(),
        ),
        // For nested relations or user-defined types, fall back to string representation or simplified object
        ScalarValue::Relation(_) => serde_json::Value::String("<Relation>".to_string()),
        ScalarValue::UserDefined { .. } => serde_json::Value::String("<UserDefined>".to_string()),
    }
}

/// Exports the relation to an ASCII table.
///
/// Generates a formatted text table suitable for terminal output or logging.
///
/// # Example
///
/// ```
/// use relvar::{tuple, experimental::exporter};
/// use relvar::types::{RelationType, TupleType, ScalarType};
/// use relvar::values::Relation;
///
/// let rel_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id", ScalarType::Int)
///         .with_attribute("name", ScalarType::String)
/// );
/// let mut relation = Relation::new(rel_type);
/// relation.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
///
/// let table = exporter::to_ascii_table(&relation);
/// println!("{}", table);
/// // +----+-------+
/// // | id | name  |
/// // +----+-------+
/// // | 1  | Alice |
/// // +----+-------+
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
    fn test_to_json_comprehensive() {
        use relvar_core::types::{RelationType, ScalarType, TupleType};
        use relvar_core::values::{Relation, ScalarValue};

        // Create a tuple type with diverse types
        let heading = TupleType::new()
            .with_attribute("float_val", ScalarType::Float)
            .with_attribute("nan_val", ScalarType::Float)
            .with_attribute("bytes_val", ScalarType::Bytes)
            .with_attribute("nested", ScalarType::Relation(Box::new(RelationType::new(TupleType::new()))))
            .with_attribute("udt", ScalarType::user_defined("MyType", ScalarType::Int));

        let mut relation = Relation::new(RelationType::new(heading));

        // Create UDT value
        let udt_type = ScalarType::user_defined("MyType", ScalarType::Int);
        let udt_val = udt_type.selector(ScalarValue::Int(123)).unwrap();

        // Create Nested Relation value
        let nested_rel = Relation::new(RelationType::new(TupleType::new()));

        relation.insert(tuple! {
            float_val: 3.14,
            nan_val: f64::NAN,
            bytes_val: vec![1u8, 2u8],
            nested: ScalarValue::Relation(nested_rel),
            udt: udt_val
        }).unwrap();

        let json = to_json(&relation).unwrap();
        println!("JSON: {}", json);

        // Assertions covering all branches
        assert!(json.contains("3.14")); // Float
        assert!(json.contains("null")); // NaN -> null
        // Check bytes content (serde pretty prints arrays)
        assert!(json.contains("1"));
        assert!(json.contains("2"));
        assert!(json.contains("\"<Relation>\"")); // Nested relation fallback
        assert!(json.contains("\"<UserDefined>\"")); // UDT fallback
    }
}
