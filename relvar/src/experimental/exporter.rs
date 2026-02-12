//! Exporter module for Relvar.
//!
//! This module provides functionality to export relations to various formats
//! like CSV, JSON, and ASCII tables.

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
/// The result is a JSON array of objects.
pub fn to_json(relation: &Relation) -> Result<String, ExporterError> {
    // Sort tuples first
    let mut tuples: Vec<SortableTuple> = relation.tuples().map(SortableTuple).collect();
    tuples.sort();

    let mut json_tuples = Vec::new();
    for tuple in tuples {
        let mut obj = serde_json::Map::new();
        // Tuple values are BTreeMap, so iteration is already sorted by key (attribute name)
        for (key, val) in tuple.0.values().iter() {
            obj.insert(key.clone(), scalar_to_json(val));
        }
        json_tuples.push(serde_json::Value::Object(obj));
    }

    serde_json::to_string_pretty(&json_tuples).map_err(ExporterError::JsonError)
}

/// Exports the relation to an ASCII table.
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

    // Pass 1: Measure data widths and format rows
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(tuples.len());
    for t in &tuples {
        let mut row: Vec<String> = Vec::with_capacity(headers.len());
        for (i, h) in headers.iter().enumerate() {
            let s = if let Some(val) = t.0.get(h) {
                format_scalar_table(val)
            } else {
                String::new()
            };
            if s.len() > widths[i] {
                widths[i] = s.len();
            }
            row.push(s);
        }
        rows.push(row);
    }

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

fn scalar_to_json(val: &ScalarValue) -> serde_json::Value {
    match val {
        ScalarValue::Int(v) => serde_json::json!(v),
        ScalarValue::Float(v) => serde_json::json!(v),
        ScalarValue::String(v) => serde_json::json!(v),
        ScalarValue::Bool(v) => serde_json::json!(v),
        ScalarValue::Bytes(v) => serde_json::json!(v),
        ScalarValue::Relation(_) => serde_json::json!("<Relation>"),
        ScalarValue::UserDefined { .. } => serde_json::json!("<UserDefined>"),
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
}
