//! Spatial data types and operations for the relational model.
//!
//! This module implements spatial primitives (Points) using the relational model's
//! composition capabilities. A `Point` is defined as a user-defined type backed by
//! a `Relation` with a single tuple `{x: Float, y: Float}`.
//!
//! This demonstrates how complex types can be built from simple scalar types
//! using the type system, without needing to extend the core engine.
//!
//! # Example
//!
//! ```
//! use relvar::experimental::spatial;
//! use relvar::values::ScalarValue;
//!
//! // Create points
//! let p1 = spatial::point(0.0, 0.0).unwrap();
//! let p2 = spatial::point(3.0, 4.0).unwrap();
//!
//! // Calculate distance
//! let dist = spatial::distance(&p1, &p2).unwrap();
//! assert_eq!(dist, 5.0);
//! ```

use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue};

/// Returns the `Point` scalar type definition.
///
/// A Point is a user-defined type backed by a Relation with heading `{x: Float, y: Float}`.
pub fn point_type() -> ScalarType {
    let heading = TupleType::new()
        .with_attribute("x", ScalarType::Float)
        .with_attribute("y", ScalarType::Float);

    // The representation is a Relation containing the coordinates
    let representation = ScalarType::Relation(Box::new(RelationType::new(heading)));

    ScalarType::user_defined("Point", representation)
}

/// Creates a new Point value.
///
/// Returns a `ScalarValue::UserDefined` wrapping a `ScalarValue::Relation`.
/// The relation contains exactly one tuple with attributes `x` and `y`.
pub fn point(x: f64, y: f64) -> Result<ScalarValue, String> {
    let pt_type = point_type();

    // Create the inner relation
    // We need to construct the relation manually since we are inside the crate
    // and might not have easy access to the tuple! macro if it's not imported correctly.
    // Let's rely on standard construction to be safe.

    let heading = TupleType::new()
        .with_attribute("x", ScalarType::Float)
        .with_attribute("y", ScalarType::Float);

    let mut relation = Relation::new(RelationType::new(heading));

    // Create tuple using BTreeMap directly to avoid macro issues if any
    let mut values = std::collections::BTreeMap::new();
    values.insert("x".to_string(), ScalarValue::Float(x));
    values.insert("y".to_string(), ScalarValue::Float(y));

    // We need to access Tuple::new, but it might verify the type.
    // Let's try to use the public API for creating tuples if possible.
    // relvar::values::Tuple::new(heading, values)

    // Wait, Tuple::new takes (TupleType, BTreeMap).
    // But we are in `relvar`, which re-exports `relvar_core`.
    // So `crate::values::Tuple` is available.

    // However, we can use the `tuple!` macro if we import it.
    // #[macro_use] extern crate relvar_core; is not how 2018 edition works.
    // It should be available if re-exported.

    // Let's use the explicit construction to be 100% safe and explicit.
    use crate::values::Tuple;
    let tuple = Tuple::new(relation.relation_type().heading().clone(), values)
        .map_err(|e| e.to_string())?;

    relation.insert(tuple).map_err(|e| e.to_string())?;

    // Wrap in UserDefined
    pt_type
        .selector(ScalarValue::Relation(relation))
        .map_err(|e| e.to_string())
}

/// Extracts coordinates from a Point value.
pub fn to_coordinates(p: &ScalarValue) -> Result<(f64, f64), String> {
    // Check type
    if p.scalar_type().name() != "Point" {
        return Err(format!("Expected Point, got {}", p.scalar_type().name()));
    }

    // Extract representation
    let inner = p.observer().map_err(|_| "Not a user defined type")?;

    match inner {
        ScalarValue::Relation(rel) => {
            if rel.cardinality() != 1 {
                return Err("Point relation must have exactly one tuple".to_string());
            }

            let tuple = rel.tuples().next().unwrap();

            let x = tuple
                .get("x")
                .ok_or("Missing x attribute")
                .and_then(|v| match v {
                    ScalarValue::Float(f) => Ok(*f),
                    _ => Err("x is not a float"),
                })?;

            let y = tuple
                .get("y")
                .ok_or("Missing y attribute")
                .and_then(|v| match v {
                    ScalarValue::Float(f) => Ok(*f),
                    _ => Err("y is not a float"),
                })?;

            Ok((x, y))
        }
        _ => Err("Point representation is not a relation".to_string()),
    }
}

/// Calculates the Euclidean distance between two points.
pub fn distance(p1: &ScalarValue, p2: &ScalarValue) -> Result<f64, String> {
    let (x1, y1) = to_coordinates(p1)?;
    let (x2, y2) = to_coordinates(p2)?;

    Ok(((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::ScalarValue;

    #[test]
    fn test_point_creation_and_type() {
        let p = point(1.5, 2.5).unwrap();
        assert_eq!(p.scalar_type().name(), "Point");

        let (x, y) = to_coordinates(&p).unwrap();
        assert_eq!(x, 1.5);
        assert_eq!(y, 2.5);
    }

    #[test]
    fn test_distance() {
        let p1 = point(0.0, 0.0).unwrap();
        let p2 = point(3.0, 4.0).unwrap();

        let dist = distance(&p1, &p2).unwrap();
        assert_eq!(dist, 5.0);
    }

    #[test]
    fn test_type_checking() {
        let p = point(0.0, 0.0).unwrap();
        let not_point = ScalarValue::Int(42);

        assert!(distance(&p, &not_point).is_err());
    }
}
