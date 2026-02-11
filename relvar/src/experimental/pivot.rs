//! Pivot operator implementation.
//!
//! PIVOT rotates a table-valued expression by turning the unique values from one column
//! in the expression into multiple columns in the output.

use relvar_core::error::DatabaseError;
use relvar_core::types::{RelationType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::collections::{BTreeMap, BTreeSet};

/// Trait extending Relation with pivot capabilities.
pub trait Pivot {
    /// Pivots a relation.
    ///
    /// # Arguments
    ///
    /// * `on_attr` - The attribute whose values will become new columns.
    /// * `value_attr` - The attribute whose values will fill the new columns.
    /// * `default_value` - The value to use for missing cells.
    ///
    /// # Returns
    ///
    /// A new relation with the pivoted structure.
    fn pivot(
        &self,
        on_attr: &str,
        value_attr: &str,
        default_value: ScalarValue,
    ) -> Result<Relation, DatabaseError>;
}

impl Pivot for Relation {
    fn pivot(
        &self,
        on_attr: &str,
        value_attr: &str,
        default_value: ScalarValue,
    ) -> Result<Relation, DatabaseError> {
        let heading = self.relation_type().heading();

        // 1. Validation
        if !heading.has_attribute(on_attr) {
            return Err(DatabaseError::AttributeNotFound(
                on_attr.to_string(),
                "relation".to_string(),
            ));
        }
        if !heading.has_attribute(value_attr) {
            return Err(DatabaseError::AttributeNotFound(
                value_attr.to_string(),
                "relation".to_string(),
            ));
        }

        // Identify grouping attributes (all attributes except on_attr and value_attr)
        let group_attrs: Vec<String> = heading
            .attribute_names()
            .filter(|&name| name != on_attr && name != value_attr)
            .cloned()
            .collect();

        // 2. Scan for new columns (unique values in on_attr)
        let mut new_columns = BTreeSet::new();
        for tuple in self.tuples() {
            let val = tuple.get(on_attr).unwrap();
            let col_name = scalar_to_string_key(val)?;
            new_columns.insert(col_name);
        }

        // Check for column name collisions with grouping attributes
        for col in &new_columns {
            if group_attrs.contains(col) {
                return Err(DatabaseError::DuplicateAttributeName(format!(
                    "Pivoted column '{}' conflicts with grouping attribute",
                    col
                )));
            }
        }

        // 3. Construct new heading
        let mut new_heading_builder = TupleType::new();

        // Add grouping attributes
        for attr in &group_attrs {
            let ty = heading.get_attribute_type(attr).unwrap();
            new_heading_builder = new_heading_builder.with_attribute(attr, ty.clone());
        }

        // Add new pivoted columns
        // The type of new columns is the type of the value_attr
        let value_type = heading.get_attribute_type(value_attr).unwrap();

        // Check if default value matches type
        if !default_value.is_type(value_type) {
            return Err(DatabaseError::AlgebraError(format!(
                "Default value type ({:?}) does not match value attribute type ({:?})",
                default_value.scalar_type(),
                value_type
            )));
        }

        for col in &new_columns {
            new_heading_builder = new_heading_builder.with_attribute(col, value_type.clone());
        }

        let new_rel_type = RelationType::new(new_heading_builder);
        let mut result_relation = Relation::new(new_rel_type.clone());

        // 4. Group data
        // Map<GroupKey, Map<ColumnName, Value>>
        // GroupKey is represented as a Vec<ScalarValue> corresponding to group_attrs
        let mut groups: BTreeMap<Vec<ScalarValue>, BTreeMap<String, ScalarValue>> = BTreeMap::new();

        // Collect and sort tuples to ensure deterministic "Last Write Wins" behavior
        // Since Relation uses HashSet (unordered), iteration order is undefined.
        // We sort by tuple content (values map) to ensure consistency.
        let mut sorted_tuples: Vec<&Tuple> = self.tuples().collect();
        sorted_tuples.sort_by(|a, b| a.values().cmp(b.values()));

        for tuple in sorted_tuples {
            let mut group_key = Vec::with_capacity(group_attrs.len());
            for attr in &group_attrs {
                group_key.push(tuple.get(attr).unwrap().clone());
            }

            let pivot_val = tuple.get(on_attr).unwrap();
            let col_name = scalar_to_string_key(pivot_val)?;

            let cell_val = tuple.get(value_attr).unwrap().clone();

            groups
                .entry(group_key)
                .or_default()
                .insert(col_name, cell_val);
        }

        // 5. Build result tuples
        for (group_key, cell_map) in groups {
            let mut tuple_values = BTreeMap::new();

            // Set grouping attributes
            for (i, attr) in group_attrs.iter().enumerate() {
                tuple_values.insert(attr.clone(), group_key[i].clone());
            }

            // Set pivoted attributes
            for col in &new_columns {
                let val = cell_map.get(col).unwrap_or(&default_value).clone();
                tuple_values.insert(col.clone(), val);
            }

            // We use Tuple::new to be safe.
            let tuple = Tuple::new(new_rel_type.heading().clone(), tuple_values).map_err(|e| {
                DatabaseError::AlgebraError(format!("Failed to construct tuple: {}", e))
            })?;

            result_relation.insert(tuple)?;
        }

        Ok(result_relation)
    }
}

fn scalar_to_string_key(val: &ScalarValue) -> Result<String, DatabaseError> {
    match val {
        ScalarValue::Int(v) => Ok(v.to_string()),
        ScalarValue::Float(_) => Err(DatabaseError::AlgebraError(
            "Cannot pivot on Float values".to_string(),
        )),
        ScalarValue::String(v) => Ok(v.clone()),
        ScalarValue::Bool(v) => Ok(v.to_string()),
        _ => Err(DatabaseError::AlgebraError(format!(
            "Cannot pivot on type {:?}",
            val.scalar_type()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_pivot_basic() {
        // Sales data: (Agent, Month, Amount)
        // A, Jan, 100
        // A, Feb, 200
        // B, Jan, 150
        let heading = TupleType::new()
            .with_attribute("Agent", ScalarType::String)
            .with_attribute("Month", ScalarType::String)
            .with_attribute("Amount", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));

        r.insert(tuple! { Agent: "A", Month: "Jan", Amount: 100i64 })
            .unwrap();
        r.insert(tuple! { Agent: "A", Month: "Feb", Amount: 200i64 })
            .unwrap();
        r.insert(tuple! { Agent: "B", Month: "Jan", Amount: 150i64 })
            .unwrap();

        let pivoted = r
            .pivot("Month", "Amount", ScalarValue::Int(0))
            .expect("Pivot failed");

        // Expected Heading: Agent, Feb, Jan (sorted alphabetically)
        let heading = pivoted.relation_type().heading();
        assert!(heading.has_attribute("Agent"));
        assert!(heading.has_attribute("Jan"));
        assert!(heading.has_attribute("Feb"));

        assert_eq!(pivoted.cardinality(), 2); // Agent A and Agent B

        // Check Agent A
        // A has Jan=100, Feb=200
        let a_tuple = pivoted
            .tuples()
            .find(|t| t.get("Agent") == Some(&ScalarValue::String("A".to_string())))
            .unwrap();
        assert_eq!(a_tuple.get("Jan"), Some(&ScalarValue::Int(100)));
        assert_eq!(a_tuple.get("Feb"), Some(&ScalarValue::Int(200)));

        // Check Agent B
        // B has Jan=150, Feb=0 (default)
        let b_tuple = pivoted
            .tuples()
            .find(|t| t.get("Agent") == Some(&ScalarValue::String("B".to_string())))
            .unwrap();
        assert_eq!(b_tuple.get("Jan"), Some(&ScalarValue::Int(150)));
        assert_eq!(b_tuple.get("Feb"), Some(&ScalarValue::Int(0)));
    }

    #[test]
    fn test_pivot_conflict() {
        // Test what happens when multiple rows map to the same cell (Last Write Wins)
        let heading = TupleType::new()
            .with_attribute("Key", ScalarType::String)
            .with_attribute("Piv", ScalarType::String)
            .with_attribute("Val", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));

        r.insert(tuple! { Key: "K1", Piv: "P1", Val: 10i64 })
            .unwrap();
        r.insert(tuple! { Key: "K1", Piv: "P1", Val: 20i64 })
            .unwrap();

        let pivoted = r
            .pivot("Piv", "Val", ScalarValue::Int(0))
            .expect("Pivot failed");

        // Should have 1 row
        assert_eq!(pivoted.cardinality(), 1);
        let t = pivoted.tuples().next().unwrap();

        // Since input order is not guaranteed in Relation iteration (it's a set),
        // we can't strictly guarantee "Last Write" relative to insertion order,
        // but we guarantee one value wins.
        // Wait, Relation stores tuples in BTreeSet, so they are ordered by value.
        // (K1, P1, 10) < (K1, P1, 20).
        // So iterator will yield 10 then 20.
        // So 20 should be the winner.
        assert_eq!(t.get("P1"), Some(&ScalarValue::Int(20)));
    }

    #[test]
    fn test_pivot_type_mismatch_default() {
        let heading = TupleType::new()
            .with_attribute("K", ScalarType::String)
            .with_attribute("P", ScalarType::String)
            .with_attribute("V", ScalarType::Int);
        let r = Relation::new(RelationType::new(heading));

        let res = r.pivot("P", "V", ScalarValue::String("wrong".to_string()));
        assert!(res.is_err());
    }

    #[test]
    fn test_pivot_invalid_pivot_type() {
        let heading = TupleType::new()
            .with_attribute("K", ScalarType::String)
            .with_attribute("P", ScalarType::Float) // Float cannot be pivot key
            .with_attribute("V", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        r.insert(tuple! { K: "A", P: 1.5, V: 10i64 }).unwrap();

        let res = r.pivot("P", "V", ScalarValue::Int(0));
        assert!(res.is_err());
    }
}
