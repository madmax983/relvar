//! Pivot operator implementation.
//!
//! PIVOT transforms row values into column headers.
//!
//! # Visual Example
//!
//! ```text
//! Before Pivot:
//! +-------+-------+-------+
//! | Item  | Color | Count |
//! +-------+-------+-------+
//! | Shirt | Red   | 10    |
//! | Shirt | Blue  | 5     |
//! | Pants | Blue  | 20    |
//! +-------+-------+-------+
//!
//! After Pivot (on Color, value Count):
//! +-------+-----+------+
//! | Item  | Red | Blue |
//! +-------+-----+------+
//! | Shirt | 10  | 5    |
//! | Pants | 0   | 20   |
//! +-------+-----+------+
//! ```
//!
//! The `pivot` operator is useful for transforming "tall" data (normalized) into "wide" data (denormalized/report format).
//!
//! # Conflict Resolution
//!
//! If multiple tuples map to the same cell (e.g., same `Item` and same `Color` in the example above),
//! the operator uses a **Last Write Wins** strategy based on the lexicographical order of the source tuples.
//! The tuple that sorts last determines the final cell value.

use relvar_core::error::DatabaseError;
use relvar_core::types::{RelationType, TupleType};
use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::collections::{BTreeMap, BTreeSet};

/// Trait extending Relation with pivot capabilities.
/// Pivots a relation.
///
/// Transforms unique values from the `on_attr` column into new column headers,
/// filling the cells with values from the `value_attr` column. All other attributes
/// become grouping keys.
///
/// # Arguments
///
/// * `relation` - The relation to pivot.
/// * `on_attr` - The attribute whose values will become new column headers.
/// * `value_attr` - The attribute whose values will populate the cells.
/// * `default_value` - The value to use when a cell is missing (e.g., `0` or `false`).
///   Must match the type of `value_attr`.
///
/// # Conflict Resolution
///
/// If multiple source tuples map to the same target cell (i.e., they have the same
/// grouping attributes and the same `on_attr` value), the **Last Write Wins** strategy
/// is applied. The source tuples are sorted, and the value from the last tuple is used.
///
/// # Examples
///
/// ```
/// use relvar_core::types::{TupleType, RelationType, ScalarType};
/// use relvar_core::values::{Relation, ScalarValue};
/// use relvar_core::tuple;
/// use relvar::experimental::pivot::pivot;
///
/// // Create a relation: (Item, Color, Count)
/// let heading = TupleType::new()
///     .with_attribute("Item", ScalarType::String)
///     .with_attribute("Color", ScalarType::String)
///     .with_attribute("Count", ScalarType::Int);
/// let mut relation = Relation::new(RelationType::new(heading));
///
/// relation.insert(tuple! { Item: "Shirt", Color: "Red", Count: 10 }).unwrap();
/// relation.insert(tuple! { Item: "Shirt", Color: "Blue", Count: 5 }).unwrap();
/// relation.insert(tuple! { Item: "Pants", Color: "Blue", Count: 20 }).unwrap();
///
/// // Pivot on 'Color', using 'Count' as values, default to 0
/// let pivoted = pivot(&relation, "Color", "Count", ScalarValue::Int(0)).unwrap();
///
/// // Result: (Item, Red, Blue)
/// // Shirt: Red=10, Blue=5
/// // Pants: Red=0 (default), Blue=20
/// ```
pub fn pivot(
    relation: &Relation,
    on_attr: &str,
    value_attr: &str,
    default_value: ScalarValue,
) -> Result<Relation, DatabaseError> {
    let heading = relation.relation_type().heading();

    let (group_attrs, new_columns) =
        scan_columns_and_validate(relation, heading, on_attr, value_attr)?;

    let value_type = heading.get_attribute_type(value_attr).unwrap();
    let new_rel_type = construct_pivot_heading(
        heading,
        &group_attrs,
        &new_columns,
        value_type,
        &default_value,
    )?;

    group_and_build_tuples(
        relation,
        on_attr,
        value_attr,
        &group_attrs,
        &new_columns,
        &default_value,
        &new_rel_type,
    )
}

fn scan_columns_and_validate(
    relation: &Relation,
    heading: &TupleType,
    on_attr: &str,
    value_attr: &str,
) -> Result<(Vec<String>, BTreeSet<String>), DatabaseError> {
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
    for tuple in relation.tuples() {
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

    Ok((group_attrs, new_columns))
}

fn construct_pivot_heading(
    heading: &TupleType,
    group_attrs: &[String],
    new_columns: &BTreeSet<String>,
    value_type: &relvar_core::types::ScalarType,
    default_value: &ScalarValue,
) -> Result<RelationType, DatabaseError> {
    let mut new_heading = TupleType::new();
    for attr in group_attrs {
        let ty = heading.get_attribute_type(attr).unwrap();
        new_heading = new_heading.with_attribute(attr, ty.clone());
    }

    if !default_value.is_type(value_type) {
        return Err(DatabaseError::AlgebraError(format!(
            "Default value type ({:?}) mismatch with value attribute ({:?})",
            default_value.scalar_type(),
            value_type
        )));
    }

    for col in new_columns {
        new_heading = new_heading.with_attribute(col, value_type.clone());
    }

    Ok(RelationType::new(new_heading))
}

fn group_and_build_tuples(
    relation: &Relation,
    on_attr: &str,
    value_attr: &str,
    group_attrs: &[String],
    new_columns: &BTreeSet<String>,
    default_value: &ScalarValue,
    new_rel_type: &RelationType,
) -> Result<Relation, DatabaseError> {
    let mut result_relation = Relation::new(new_rel_type.clone());

    // Group data
    let mut groups: BTreeMap<Vec<ScalarValue>, BTreeMap<String, ScalarValue>> = BTreeMap::new();

    // Sort tuples for deterministic Last Write Wins
    let mut sorted_tuples: Vec<&Tuple> = relation.tuples().collect();
    sorted_tuples.sort_by(|a, b| a.values().cmp(b.values()));

    for tuple in sorted_tuples {
        let mut group_key = Vec::with_capacity(group_attrs.len());
        for attr in group_attrs {
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
        for col in new_columns {
            let val = cell_map.get(col).unwrap_or(default_value).clone();
            tuple_values.insert(col.clone(), val);
        }
        let tuple = Tuple::new(new_rel_type.heading().clone(), tuple_values)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        result_relation.insert(tuple)?;
    }

    Ok(result_relation)
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

        let p = pivot(&r, "M", "V", ScalarValue::Int(0)).unwrap();

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

        let p = pivot(&r, "P", "V", ScalarValue::Int(0)).unwrap();
        assert_eq!(p.cardinality(), 1);
        let t = p.tuples().next().unwrap();
        assert_eq!(t.get("P1"), Some(&ScalarValue::Int(20)));
    }

    #[test]
    fn test_pivot_errors() {
        use relvar_core::error::DatabaseError;
        let heading = TupleType::new()
            .with_attribute("A", ScalarType::String)
            .with_attribute("M", ScalarType::String)
            .with_attribute("V", ScalarType::Int)
            .with_attribute("Jan", ScalarType::String);
        let mut r = Relation::new(RelationType::new(heading));
        r.insert(tuple! { A: "X", M: "Jan", V: 10, Jan: "Y" })
            .unwrap();

        // Missing on_attr
        let err = pivot(&r, "Missing", "V", ScalarValue::Int(0)).unwrap_err();
        assert!(matches!(err, DatabaseError::AttributeNotFound(..)));

        // Missing value_attr
        let err = pivot(&r, "M", "Missing", ScalarValue::Int(0)).unwrap_err();
        assert!(matches!(err, DatabaseError::AttributeNotFound(..)));

        // Conflicting column name
        let err = pivot(&r, "M", "V", ScalarValue::Int(0)).unwrap_err();
        assert!(matches!(err, DatabaseError::DuplicateAttributeName(..)));

        // Type mismatch for default value
        let heading2 = TupleType::new()
            .with_attribute("A", ScalarType::String)
            .with_attribute("M", ScalarType::String)
            .with_attribute("V", ScalarType::Int);
        let mut r2 = Relation::new(RelationType::new(heading2));
        r2.insert(tuple! { A: "X", M: "Jan", V: 10 }).unwrap();

        let err = pivot(&r2, "M", "V", ScalarValue::String("0".to_string())).unwrap_err();
        assert!(matches!(err, DatabaseError::AlgebraError(..)));

        // Unsupported type for pivot key
        let heading3 = TupleType::new()
            .with_attribute("A", ScalarType::String)
            .with_attribute("M", ScalarType::Float)
            .with_attribute("V", ScalarType::Int);
        let mut r3 = Relation::new(RelationType::new(heading3));
        r3.insert(tuple! { A: "X", M: 1.0, V: 10 }).unwrap();
        let err = pivot(&r3, "M", "V", ScalarValue::Int(0)).unwrap_err();
        assert!(matches!(err, DatabaseError::AlgebraError(..)));
    }
}
