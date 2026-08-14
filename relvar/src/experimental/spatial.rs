//! Experimental Spatial Types.
//!
//! This module demonstrates how to implement complex types (like `Point`) using
//! the Relational Model's support for Relation-Valued Attributes (RVAs) and
//! User-Defined Types (UDTs).
//!
//! # Philosophy: Pure Relational Types
//!
//! Instead of introducing an opaque binary `Point` type (as in PostGIS), we define
//! a `Point` as a **User-Defined Type (UDT)** that wraps a **Relation** with a specific heading:
//!
//! ```text
//! Point Type Definition:
//! POSSREP {
//!     x: Float,
//!     y: Float
//! }
//! ```
//!
//! Internally, this is represented as a relation with heading `{x: Float, y: Float}`
//! constrained to have exactly **cardinality 1**.
//!
//! This approach ensures that the internal structure of the type is visible to the
//! relational engine, allowing us to query components (e.g., `p.x`) using standard
//! relational operators like `UNGROUP` or `EXTEND`.

use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// The name of the Point type.
pub const POINT_TYPE_NAME: &str = "Point";

/// Defines and provides the required `ScalarType` representing a spatial point.
///
/// A Point is a User-Defined Type wrapping a Relation with heading `{x: Float, y: Float}`.
///
/// # Examples
///
/// ```text
/// // Example
/// ```
pub fn point_type() -> ScalarType {
    let heading = TupleType::new()
        .with_attribute("x", ScalarType::Float)
        .with_attribute("y", ScalarType::Float);

    let representation = ScalarType::Relation(Box::new(RelationType::new(heading)));

    ScalarType::user_defined(POINT_TYPE_NAME, representation)
}

/// Constructs a Point value.
///
/// Creates a relation with a single tuple `{x: x, y: y}` and wraps it in the
/// Point user-defined type.
///
/// # Examples
///
/// ```
/// use relvar::experimental::spatial::point;
/// use relvar_core::values::ScalarValue;
///
/// let p = point(3.0, 4.0);
/// assert_eq!(p.scalar_type().name(), "Point");
/// ```
pub fn point(x: f64, y: f64) -> ScalarValue {
    let pt_type = point_type();

    // Extract the underlying relation type from the user-defined type
    let rel_type = match &pt_type {
        ScalarType::UserDefined { representation, .. } => match &**representation {
            ScalarType::Relation(rt) => rt.as_ref().clone(),
            _ => unreachable!("Point representation must be a Relation"),
        },
        _ => unreachable!("Point type must be UserDefined"),
    };

    let mut rel = Relation::new(rel_type);
    // Insert the single tuple
    rel.insert(tuple! { x: x, y: y }).unwrap();

    // Wrap in UserDefined
    ScalarValue::UserDefined {
        type_def: pt_type,
        value: Box::new(ScalarValue::Relation(rel)),
    }
}

/// Calculates the Euclidean distance between two Points.
///
/// # Examples
///
/// ```
/// use relvar::experimental::spatial::{point, distance};
///
/// let p1 = point(0.0, 0.0);
/// let p2 = point(3.0, 4.0);
///
/// let dist = distance(&p1, &p2).unwrap();
/// assert_eq!(dist, 5.0);
/// ```
pub fn distance(p1: &ScalarValue, p2: &ScalarValue) -> Result<f64, String> {
    let pt_type = point_type();

    // Type check
    if !p1.is_type(&pt_type) {
        return Err(format!("Expected Point, got {:?}", p1.scalar_type().name()));
    }
    if !p2.is_type(&pt_type) {
        return Err(format!("Expected Point, got {:?}", p2.scalar_type().name()));
    }

    // Extract coordinates
    let (x1, y1) = extract_coords(p1)?;
    let (x2, y2) = extract_coords(p2)?;

    Ok(((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt())
}

/// Checks if a point is within a given radius of a center point.
///
/// # Examples
///
/// ```
/// use relvar::experimental::spatial::{point, within};
///
/// let center = point(0.0, 0.0);
/// let p = point(3.0, 4.0);
///
/// assert!(within(&p, &center, 5.0).unwrap());
/// assert!(!within(&p, &center, 4.0).unwrap());
/// ```
pub fn within(p: &ScalarValue, center: &ScalarValue, radius: f64) -> Result<bool, String> {
    let dist = distance(p, center)?;
    Ok(dist <= radius)
}

/// Helper to extract (x, y) from a Point value.
fn extract_coords(p: &ScalarValue) -> Result<(f64, f64), String> {
    // Unwrap UserDefined wrapper
    let inner_val = p.observer().map_err(|_| "Not a user-defined type")?;

    match inner_val {
        ScalarValue::Relation(ref rel) => {
            if rel.cardinality() != 1 {
                return Err("Point relation must have exactly 1 tuple".to_string());
            }
            let tuple = rel.tuples().next().unwrap();
            let x = tuple.get_typed::<f64>("x").ok_or("Missing x coordinate")?;
            let y = tuple.get_typed::<f64>("y").ok_or("Missing y coordinate")?;
            Ok((x, y))
        }
        _ => Err("Point representation must be a Relation".to_string()),
    }
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_creation_and_type() {
        let p = point(3.0, 4.0);
        assert_eq!(p.scalar_type().name(), POINT_TYPE_NAME);

        let (x, y) = extract_coords(&p).unwrap();
        assert_eq!(x, 3.0);
        assert_eq!(y, 4.0);
    }

    #[test]
    fn test_distance() {
        let p1 = point(0.0, 0.0);
        let p2 = point(3.0, 4.0);

        let dist = distance(&p1, &p2).unwrap();
        assert_eq!(dist, 5.0);
    }

    #[test]
    fn test_within() {
        let center = point(0.0, 0.0);
        let p1 = point(3.0, 4.0); // dist 5
        let p2 = point(10.0, 10.0); // dist > 10

        assert!(within(&p1, &center, 5.0).unwrap());
        assert!(within(&p1, &center, 6.0).unwrap());
        assert!(!within(&p1, &center, 4.9).unwrap());
        assert!(!within(&p2, &center, 5.0).unwrap());
    }

    #[test]
    fn test_type_safety() {
        let p = point(1.0, 1.0);
        let not_a_point = ScalarValue::Int(42);

        assert!(distance(&p, &not_a_point).is_err());
        assert!(within(&not_a_point, &p, 10.0).is_err());
    }
}
