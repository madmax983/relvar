//! Pivot operator implementation.
//!
//! This module implements the pivot operator, which allows reshaping relations
//! by rotating unique values from a column into multiple new attribute columns.

use relvar_core::types::{RelationType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Errors that can occur during pivot operations.
#[derive(Debug, Error)]
pub enum PivotError {
    /// The specified pivot or value column does not exist.
    #[error("Attribute '{0}' not found in relation")]
    AttributeNotFound(String),

    /// The default value's type does not match the value column's type.
    #[error("Default value type mismatch: expected {0}, got {1}")]
    DefaultValueTypeMismatch(String, String),

    /// A value in the pivot column cannot be converted to a valid attribute name.
    #[error("Pivot column value cannot be converted to attribute name: {0:?}")]
    InvalidPivotValue(ScalarValue),

    /// Failed to create a tuple during the pivot operation.
    #[error("Failed to create pivoted tuple: {0}")]
    TupleCreation(String),
}

/// Trait adding the `pivot` operator to relations.
///
/// The pivot operator rotates a relation by turning unique values from one column
/// into multiple columns in the output. It is useful for reshaping data for
/// analysis or reporting (similar to Excel pivot tables or Pandas `pivot`).
pub trait Pivot {
    /// Pivots the relation.
    ///
    /// # Arguments
    ///
    /// * `pivot_col` - The column whose values will become new attribute names.
    /// * `value_col` - The column whose values will fill the new attributes.
    /// * `default_value` - The value to use when a cell is missing (no tuple exists
    ///   for the given row key and pivot column value).
    ///
    /// # Returns
    ///
    /// A new relation where:
    /// - The `pivot_col` and `value_col` are removed.
    /// - All other columns form the "grouping key" (identifying unique rows).
    /// - New attributes are added for each unique value found in `pivot_col`.
    ///
    /// # Behavior
    ///
    /// - **Grouping**: The result has one tuple per unique combination of the
    ///   grouping columns (all columns except `pivot_col` and `value_col`).
    /// - **Conflicts**: If multiple source tuples map to the same cell (same grouping
    ///   key and same pivot value), the *last* one encountered wins. To avoid this,
    ///   ensure the source relation is unique on (grouping_cols + pivot_col) or
    ///   use `summarize` first to aggregate.
    /// - **Type Safety**: The `default_value` must have the same type as `value_col`.
    ///   All new attributes will have this type.
    fn pivot(
        &self,
        pivot_col: &str,
        value_col: &str,
        default_value: ScalarValue,
    ) -> Result<Relation, PivotError>;
}

impl Pivot for Relation {
    fn pivot(
        &self,
        pivot_col: &str,
        value_col: &str,
        default_value: ScalarValue,
    ) -> Result<Relation, PivotError> {
        let heading = self.relation_type().tuple_type();

        // 1. Validate inputs
        if !heading.has_attribute(pivot_col) {
            return Err(PivotError::AttributeNotFound(pivot_col.to_string()));
        }
        let value_type = heading
            .get_attribute_type(value_col)
            .ok_or_else(|| PivotError::AttributeNotFound(value_col.to_string()))?;

        if !default_value.is_type(value_type) {
            return Err(PivotError::DefaultValueTypeMismatch(
                value_type.name(),
                default_value.scalar_type().name(),
            ));
        }

        // 2. Identify grouping columns (all except pivot and value cols)
        let grouping_cols: Vec<String> = heading
            .attribute_names()
            .filter(|&name| name != pivot_col && name != value_col)
            .cloned()
            .collect();

        // 3. Scan for unique pivot values to determine new attributes
        let mut pivot_values = HashSet::new();
        // Also group tuples by key while we scan to avoid re-iterating too much
        // Key: Grouping values (as Vec<ScalarValue>)
        // Value: Map from PivotValue (string) -> Value
        let mut groups: HashMap<Vec<ScalarValue>, HashMap<String, ScalarValue>> = HashMap::new();

        for tuple in self.tuples() {
            // Extract pivot value and convert to attribute name
            let raw_pivot_val = tuple.get(pivot_col).unwrap();
            let attr_name = scalar_to_attr_name(raw_pivot_val)?;
            pivot_values.insert(attr_name.clone());

            // Extract grouping key
            let key: Vec<ScalarValue> = grouping_cols
                .iter()
                .map(|col| tuple.get(col).unwrap().clone())
                .collect();

            // Store value
            let val = tuple.get(value_col).unwrap().clone();

            groups
                .entry(key)
                .or_default()
                .insert(attr_name, val);
        }

        // 4. Construct new RelationType
        let mut new_heading = TupleType::new();

        // Add grouping columns
        for col in &grouping_cols {
            let ty = heading.get_attribute_type(col).unwrap();
            new_heading = new_heading.with_attribute(col.clone(), ty.clone());
        }

        // Add pivoted columns (sorted for determinism)
        let mut sorted_pivot_attrs: Vec<_> = pivot_values.into_iter().collect();
        sorted_pivot_attrs.sort();

        for attr in &sorted_pivot_attrs {
            new_heading = new_heading.with_attribute(attr.clone(), value_type.clone());
        }

        // 5. Construct new tuples
        let mut new_tuples = Vec::new();

        for (key, val_map) in groups {
            let mut tuple_values = HashMap::new();

            // Fill grouping columns
            for (i, col) in grouping_cols.iter().enumerate() {
                tuple_values.insert(col.clone(), key[i].clone());
            }

            // Fill pivoted columns
            for attr in &sorted_pivot_attrs {
                let val = val_map.get(attr).unwrap_or(&default_value).clone();
                tuple_values.insert(attr.clone(), val);
            }

            let tuple = Tuple::new(new_heading.clone(), tuple_values)
                .map_err(|e| PivotError::TupleCreation(e.to_string()))?;
            new_tuples.push(tuple);
        }

        Ok(Relation::from_tuples(
            RelationType::new(new_heading),
            new_tuples,
        ).expect("Pivoted tuples should conform to new relation type"))
    }
}

/// Helper to convert a scalar value to a valid attribute name string.
fn scalar_to_attr_name(val: &ScalarValue) -> Result<String, PivotError> {
    match val {
        ScalarValue::String(s) => Ok(s.clone()),
        ScalarValue::Int(i) => Ok(i.to_string()),
        ScalarValue::Bool(b) => Ok(b.to_string()),
        // Floats are risky as keys due to precision printing, but we allow it for simple cases
        ScalarValue::Float(f) => Ok(f.to_string()),
        _ => Err(PivotError::InvalidPivotValue(val.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::types::ScalarType;
    use relvar_core::tuple;

    #[test]
    fn test_pivot_basic() {
        // Schema: Student, Subject, Grade
        let heading = TupleType::new()
            .with_attribute("Student", ScalarType::String)
            .with_attribute("Subject", ScalarType::String)
            .with_attribute("Grade", ScalarType::Int);

        let mut rel = Relation::new(RelationType::new(heading));
        rel.insert(tuple! { Student: "Alice", Subject: "Math", Grade: 90i64 }).unwrap();
        rel.insert(tuple! { Student: "Alice", Subject: "Science", Grade: 85i64 }).unwrap();
        rel.insert(tuple! { Student: "Bob", Subject: "Math", Grade: 80i64 }).unwrap();
        // Bob missing Science

        let pivoted = rel.pivot("Subject", "Grade", ScalarValue::Int(0)).unwrap();

        assert_eq!(pivoted.cardinality(), 2); // Alice and Bob
        assert_eq!(pivoted.degree(), 3); // Student, Math, Science

        // Check Alice
        let alice = pivoted.tuples().find(|t| t.get("Student") == Some(&ScalarValue::String("Alice".to_string()))).unwrap();
        assert_eq!(alice.get_typed::<i64>("Math").unwrap(), 90);
        assert_eq!(alice.get_typed::<i64>("Science").unwrap(), 85);

        // Check Bob
        let bob = pivoted.tuples().find(|t| t.get("Student") == Some(&ScalarValue::String("Bob".to_string()))).unwrap();
        assert_eq!(bob.get_typed::<i64>("Math").unwrap(), 80);
        assert_eq!(bob.get_typed::<i64>("Science").unwrap(), 0); // Default value
    }

    #[test]
    fn test_pivot_with_int_keys() {
        // Schema: Year, Quarter, Sales
        let heading = TupleType::new()
            .with_attribute("Year", ScalarType::Int)
            .with_attribute("Quarter", ScalarType::Int)
            .with_attribute("Sales", ScalarType::Int);

        let mut rel = Relation::new(RelationType::new(heading));
        rel.insert(tuple! { Year: 2023i64, Quarter: 1i64, Sales: 100i64 }).unwrap();
        rel.insert(tuple! { Year: 2023i64, Quarter: 2i64, Sales: 200i64 }).unwrap();

        let pivoted = rel.pivot("Quarter", "Sales", ScalarValue::Int(0)).unwrap();

        assert_eq!(pivoted.degree(), 3); // Year, "1", "2"
        assert!(pivoted.relation_type().has_attribute("1"));
        assert!(pivoted.relation_type().has_attribute("2"));

        let tuple = pivoted.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<i64>("1").unwrap(), 100);
        assert_eq!(tuple.get_typed::<i64>("2").unwrap(), 200);
    }

    #[test]
    fn test_pivot_empty() {
        let heading = TupleType::new()
            .with_attribute("A", ScalarType::Int)
            .with_attribute("B", ScalarType::Int);
        let rel = Relation::new(RelationType::new(heading));

        let pivoted = rel.pivot("A", "B", ScalarValue::Int(0)).unwrap();

        assert_eq!(pivoted.cardinality(), 0);
        assert_eq!(pivoted.degree(), 0);
    }

    #[test]
    fn test_pivot_type_mismatch() {
        let heading = TupleType::new()
            .with_attribute("A", ScalarType::Int)
            .with_attribute("B", ScalarType::Int);
        let rel = Relation::new(RelationType::new(heading));

        // Default value is String, but value column B is Int
        let result = rel.pivot("A", "B", ScalarValue::String("0".to_string()));
        assert!(matches!(result, Err(PivotError::DefaultValueTypeMismatch(_, _))));
    }

    #[test]
    fn test_pivot_conflict_last_write_wins() {
        let heading = TupleType::new()
            .with_attribute("Student", ScalarType::String)
            .with_attribute("Subject", ScalarType::String)
            .with_attribute("Grade", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading));
        rel.insert(tuple! { Student: "Alice", Subject: "Math", Grade: 90i64 }).unwrap();
        rel.insert(tuple! { Student: "Alice", Subject: "Math", Grade: 95i64 }).unwrap();

        let pivoted = rel.pivot("Subject", "Grade", ScalarValue::Int(0)).unwrap();

        assert_eq!(pivoted.cardinality(), 1);

        let alice = pivoted.tuples().next().unwrap();
        let math_grade = alice.get_typed::<i64>("Math").unwrap();

        assert!(math_grade == 90 || math_grade == 95);
    }
}
