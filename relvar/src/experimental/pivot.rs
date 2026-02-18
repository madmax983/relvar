//! Pivot operator implementation.
//!
//! PIVOT transforms row values into column headers, rotating data from a "tall"
//! format to a "wide" format.
//!
//! # Visual Example
//!
//! **Input Relation (Tall):**
//!
//! | Product | Month | Sales |
//! |---------|-------|-------|
//! | A       | Jan   | 100   |
//! | A       | Feb   | 200   |
//! | B       | Jan   | 300   |
//!
//! **Pivoted Relation (Wide):**
//!
//! `pivot("Month", "Sales", 0)`
//!
//! | Product | Jan | Feb |
//! |---------|-----|-----|
//! | A       | 100 | 200 |
//! | B       | 300 | 0   |
//!
//! # Conflict Resolution
//!
//! If multiple source tuples map to the same cell in the pivoted table (e.g.
//! same Product and Month), the value from the **last** tuple (in sorted order)
//! determines the cell value. This is a "Last Write Wins" strategy based on
//! tuple sorting order.

use relvar_core::error::DatabaseError;
use relvar_core::types::{RelationType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::collections::{BTreeMap, BTreeSet};

/// Trait extending Relation with pivot capabilities.
pub trait Pivot {
    /// Pivots a relation.
    ///
    /// Transforms unique values from `on_attr` into new columns,
    /// filling cells with values from `value_attr`.
    ///
    /// The remaining attributes (those not used for `on_attr` or `value_attr`)
    /// become the grouping key (identifying the rows).
    ///
    /// # Arguments
    ///
    /// * `on_attr` - Attribute whose values will become new column headers.
    /// * `value_attr` - Attribute whose values will fill the cells.
    /// * `default_value` - Value to use for missing cells (must match `value_attr` type).
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError` if:
    /// - Attributes don't exist.
    /// - Default value type mismatch.
    /// - Pivoted column name conflicts with existing attribute.
    fn pivot(
        &self,
        on_attr: &str,
        value_attr: &str,
        default_value: ScalarValue,
    ) -> Result<Relation, DatabaseError>;
}

impl Pivot for Relation {
    /// Implementation of pivot for [`Relation`].
    ///
    /// # Algorithm
    ///
    /// 1. Scan `on_attr` to determine new column names.
    /// 2. Construct new relation heading: grouping attributes + new columns.
    /// 3. Sort input tuples (for deterministic conflict resolution).
    /// 4. Iterate tuples, filling a map of `group_key -> {col_name -> value}`.
    /// 5. Flatten map into new tuples.
    ///
    /// # Conflict Resolution
    ///
    /// "Last Write Wins": If multiple tuples have the same grouping key and
    /// pivot value, the one appearing later in the sorted order overwrites previous ones.
    fn pivot(
        &self,
        on_attr: &str,
        value_attr: &str,
        default_value: ScalarValue,
    ) -> Result<Relation, DatabaseError> {
        let heading = self.relation_type().heading();

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

        let group_attrs: Vec<String> = heading
            .attribute_names()
            .filter(|&name| name != on_attr && name != value_attr)
            .cloned()
            .collect();

        // Scan for new columns
        let mut new_columns = BTreeSet::new();
        for tuple in self.tuples() {
            let val = tuple.get(on_attr).unwrap();
            let col_name = scalar_to_string_key(val)?;
            new_columns.insert(col_name);
        }

        // Check collisions
        for col in &new_columns {
            if group_attrs.contains(col) {
                return Err(DatabaseError::DuplicateAttributeName(format!(
                    "Pivoted column '{}' conflicts with grouping attribute",
                    col
                )));
            }
        }

        // Construct new heading
        let mut new_heading = TupleType::new();
        for attr in &group_attrs {
            let ty = heading.get_attribute_type(attr).unwrap();
            new_heading = new_heading.with_attribute(attr, ty.clone());
        }

        let value_type = heading.get_attribute_type(value_attr).unwrap();
        if !default_value.is_type(value_type) {
            return Err(DatabaseError::AlgebraError(format!(
                "Default value type ({:?}) mismatch with value attribute ({:?})",
                default_value.scalar_type(),
                value_type
            )));
        }

        for col in &new_columns {
            new_heading = new_heading.with_attribute(col, value_type.clone());
        }

        let new_rel_type = RelationType::new(new_heading);
        let mut result_relation = Relation::new(new_rel_type.clone());

        // Group data
        let mut groups: BTreeMap<Vec<ScalarValue>, BTreeMap<String, ScalarValue>> = BTreeMap::new();

        // Sort tuples for deterministic Last Write Wins
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

        // Build result tuples
        for (group_key, cell_map) in groups {
            let mut tuple_values = BTreeMap::new();
            for (i, attr) in group_attrs.iter().enumerate() {
                tuple_values.insert(attr.clone(), group_key[i].clone());
            }
            for col in &new_columns {
                let val = cell_map.get(col).unwrap_or(&default_value).clone();
                tuple_values.insert(col.clone(), val);
            }
            let tuple = Tuple::new(new_rel_type.heading().clone(), tuple_values)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
            result_relation.insert(tuple)?;
        }

        Ok(result_relation)
    }
}

fn scalar_to_string_key(val: &ScalarValue) -> Result<String, DatabaseError> {
    match val {
        ScalarValue::Int(v) => Ok(v.to_string()),
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
        let heading = TupleType::new()
            .with_attribute("A", ScalarType::String)
            .with_attribute("M", ScalarType::String)
            .with_attribute("V", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));

        r.insert(tuple! { A: "X", M: "Jan", V: 10 }).unwrap();
        r.insert(tuple! { A: "X", M: "Feb", V: 20 }).unwrap();
        r.insert(tuple! { A: "Y", M: "Jan", V: 30 }).unwrap();

        let p = r.pivot("M", "V", ScalarValue::Int(0)).unwrap();

        // Expected: A, Jan, Feb
        assert_eq!(p.cardinality(), 2);

        let t_x = p
            .tuples()
            .find(|t| t.get("A") == Some(&ScalarValue::String("X".into())))
            .unwrap();
        assert_eq!(t_x.get("Jan"), Some(&ScalarValue::Int(10)));
        assert_eq!(t_x.get("Feb"), Some(&ScalarValue::Int(20)));

        let t_y = p
            .tuples()
            .find(|t| t.get("A") == Some(&ScalarValue::String("Y".into())))
            .unwrap();
        assert_eq!(t_y.get("Jan"), Some(&ScalarValue::Int(30)));
        assert_eq!(t_y.get("Feb"), Some(&ScalarValue::Int(0))); // Default
    }

    #[test]
    fn test_pivot_conflict() {
        let heading = TupleType::new()
            .with_attribute("K", ScalarType::String)
            .with_attribute("P", ScalarType::String)
            .with_attribute("V", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));

        // (K1, P1, 10) < (K1, P1, 20) -> 20 wins (last write)
        r.insert(tuple! { K: "K1", P: "P1", V: 10 }).unwrap();
        r.insert(tuple! { K: "K1", P: "P1", V: 20 }).unwrap();

        let p = r.pivot("P", "V", ScalarValue::Int(0)).unwrap();
        assert_eq!(p.cardinality(), 1);
        let t = p.tuples().next().unwrap();
        assert_eq!(t.get("P1"), Some(&ScalarValue::Int(20)));
    }
}
