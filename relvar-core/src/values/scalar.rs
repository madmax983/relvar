//! Scalar values for the relational model.
//!
//! This module defines [`ScalarValue`], which represents atomic runtime values
//! that can be stored as attribute values in tuples.
//!
//! # TTM Compliance
//!
//! - All values carry their type (introspection is possible)
//! - No NULL values are permitted
//!
//! # Example
//!
//! ```
//! use relvar_core::values::ScalarValue;
//! use relvar_core::types::ScalarType;
//!
//! // Built-in values
//! let age = ScalarValue::Int(42);
//! let name = ScalarValue::String("Alice".to_string());
//! let pi = ScalarValue::Float(3.14159);
//! let active = ScalarValue::Bool(true);
//!
//! // Check types
//! assert!(age.is_type(&ScalarType::Int));
//! assert!(name.is_type(&ScalarType::String));
//! ```

use crate::types::ScalarType;
use serde::{Deserialize, Serialize};

/// Represents an atomic (scalar) value at runtime.
///
/// Each `ScalarValue` holds data of a specific type and can report its type
/// via the [`scalar_type()`](Self::scalar_type) method. This implements the
/// TTM principle that all values carry their type.
///
/// # Built-in Value Types
///
/// - [`Int`](ScalarValue::Int) - 64-bit signed integer (`i64`)
/// - [`Float`](ScalarValue::Float) - 64-bit floating point (`f64`)
/// - [`String`](ScalarValue::String) - UTF-8 string
/// - [`Bool`](ScalarValue::Bool) - Boolean
/// - [`Bytes`](ScalarValue::Bytes) - Byte sequence
///
/// # Advanced Values
///
/// - [`Relation`](ScalarValue::Relation) - Nested relation (for RVAs)
///
/// # Equality and Hashing
///
/// `ScalarValue` implements `Eq` and `Hash` to support use in sets and as
/// hash map keys. Notably, floating-point values use bit equality, which
/// means `NaN == NaN` (required for database set semantics).
///
/// # Example
///
/// ```
/// use relvar_core::values::ScalarValue;
/// use relvar_core::types::ScalarType;
///
/// let value = ScalarValue::Int(100);
///
/// // Introspect the type
/// assert_eq!(value.scalar_type(), ScalarType::Int);
/// assert!(value.is_type(&ScalarType::Int));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScalarValue {
    /// 64-bit signed integer value.
    Int(i64),

    /// 64-bit floating point value.
    ///
    /// Uses bit equality for comparison, so `NaN == NaN` for set semantics.
    Float(f64),

    /// UTF-8 string value.
    String(String),

    /// Boolean value.
    Bool(bool),

    /// Arbitrary byte sequence.
    Bytes(Vec<u8>),

    /// Relation value (for relation-valued attributes).
    ///
    /// Enables nested relations within tuples.
    Relation(crate::values::Relation),
}

impl ScalarValue {
    /// Returns the scalar type of this value.
    ///
    /// Every value in the relational model carries its type. This method
    /// allows introspection of the value's type at runtime.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::values::ScalarValue;
    /// use relvar_core::types::ScalarType;
    ///
    /// let value = ScalarValue::Int(42);
    /// assert_eq!(value.scalar_type(), ScalarType::Int);
    ///
    /// let value = ScalarValue::String("hello".to_string());
    /// assert_eq!(value.scalar_type(), ScalarType::String);
    /// ```
    pub fn scalar_type(&self) -> ScalarType {
        match self {
            ScalarValue::Int(_) => ScalarType::Int,
            ScalarValue::Float(_) => ScalarType::Float,
            ScalarValue::String(_) => ScalarType::String,
            ScalarValue::Bool(_) => ScalarType::Bool,
            ScalarValue::Bytes(_) => ScalarType::Bytes,
            ScalarValue::Relation(rel) => {
                ScalarType::Relation(Box::new(rel.relation_type().clone()))
            }
        }
    }

    /// Checks if this value is of the given type
    pub fn is_type(&self, ty: &ScalarType) -> bool {
        &self.scalar_type() == ty
    }
}

// Custom PartialEq implementation for ScalarValue
// Note: Float comparison uses bit equality, which is appropriate for
// database values (we want NaN == NaN for set semantics)
impl PartialEq for ScalarValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ScalarValue::Int(a), ScalarValue::Int(b)) => a == b,
            (ScalarValue::Float(a), ScalarValue::Float(b)) => {
                if a.is_nan() && b.is_nan() {
                    true
                } else {
                    a.to_bits() == b.to_bits()
                }
            }
            (ScalarValue::String(a), ScalarValue::String(b)) => a == b,
            (ScalarValue::Bool(a), ScalarValue::Bool(b)) => a == b,
            (ScalarValue::Bytes(a), ScalarValue::Bytes(b)) => a == b,
            (ScalarValue::Relation(a), ScalarValue::Relation(b)) => a == b,
            _ => false,
        }
    }
}

// Custom Eq implementation for ScalarValue
impl Eq for ScalarValue {}

// Custom Hash implementation
impl std::hash::Hash for ScalarValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ScalarValue::Int(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            ScalarValue::Float(v) => {
                1u8.hash(state);
                if v.is_nan() {
                    f64::NAN.to_bits().hash(state);
                } else {
                    v.to_bits().hash(state);
                }
            }
            ScalarValue::String(v) => {
                2u8.hash(state);
                v.hash(state);
            }
            ScalarValue::Bool(v) => {
                3u8.hash(state);
                v.hash(state);
            }
            ScalarValue::Bytes(v) => {
                4u8.hash(state);
                v.hash(state);
            }
            ScalarValue::Relation(v) => {
                5u8.hash(state);
                v.hash(state);
            }
        }
    }
}

// Custom PartialOrd implementation for MIN/MAX operations
impl PartialOrd for ScalarValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// Custom Ord implementation for use in BTreeMap
// We order by type first, then by value within type
impl Ord for ScalarValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        // Helper to get type ordering
        fn type_order(val: &ScalarValue) -> u8 {
            match val {
                ScalarValue::Int(_) => 0,
                ScalarValue::Float(_) => 1,
                ScalarValue::String(_) => 2,
                ScalarValue::Bool(_) => 3,
                ScalarValue::Bytes(_) => 4,
                ScalarValue::Relation(_) => 5,
            }
        }

        // Compare types first
        match type_order(self).cmp(&type_order(other)) {
            Ordering::Equal => {
                // Same type: order by value
                match (self, other) {
                    (ScalarValue::Int(a), ScalarValue::Int(b)) => a.cmp(b),
                    (ScalarValue::Float(a), ScalarValue::Float(b)) => {
                        // For floats, treat all NaNs as equal and greater than any other float
                        match (a.is_nan(), b.is_nan()) {
                            (true, true) => Ordering::Equal,
                            (true, false) => Ordering::Greater,
                            (false, true) => Ordering::Less,
                            (false, false) => a.to_bits().cmp(&b.to_bits()),
                        }
                    }
                    (ScalarValue::String(a), ScalarValue::String(b)) => a.cmp(b),
                    (ScalarValue::Bool(a), ScalarValue::Bool(b)) => a.cmp(b),
                    (ScalarValue::Bytes(a), ScalarValue::Bytes(b)) => a.cmp(b),
                    (ScalarValue::Relation(a), ScalarValue::Relation(b)) => {
                        // For relations, order by cardinality first, then degree
                        match a.cardinality().cmp(&b.cardinality()) {
                            Ordering::Equal => a.degree().cmp(&b.degree()),
                            other => other,
                        }
                    }
                    _ => unreachable!("Type orders are equal but types don't match"),
                }
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_values_carry_their_type() {
        let int_val = ScalarValue::Int(42);
        let float_val = ScalarValue::Float(3.5);
        let string_val = ScalarValue::String("hello".to_string());
        let bool_val = ScalarValue::Bool(true);
        let bytes_val = ScalarValue::Bytes(vec![1, 2, 3]);

        assert_eq!(int_val.scalar_type(), ScalarType::Int);
        assert_eq!(float_val.scalar_type(), ScalarType::Float);
        assert_eq!(string_val.scalar_type(), ScalarType::String);
        assert_eq!(bool_val.scalar_type(), ScalarType::Bool);
        assert_eq!(bytes_val.scalar_type(), ScalarType::Bytes);
    }

    #[test]
    fn test_values_of_same_type_can_be_compared() {
        let int1 = ScalarValue::Int(42);
        let int2 = ScalarValue::Int(42);
        let int3 = ScalarValue::Int(43);

        assert_eq!(int1, int2);
        assert_ne!(int1, int3);

        let str1 = ScalarValue::String("hello".to_string());
        let str2 = ScalarValue::String("hello".to_string());
        let str3 = ScalarValue::String("world".to_string());

        assert_eq!(str1, str2);
        assert_ne!(str1, str3);
    }

    #[test]
    fn test_values_of_different_types_are_not_equal() {
        let int_val = ScalarValue::Int(42);
        let float_val = ScalarValue::Float(42.0);

        assert_ne!(int_val, float_val);
    }

    #[test]
    fn test_values_can_be_serialized_and_deserialized() {
        let original = ScalarValue::Int(42);
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original, deserialized);

        let original = ScalarValue::String("test".to_string());
        let serialized = serde_json::to_string(&original).unwrap();
        let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_float_equality_for_set_semantics() {
        // For database semantics, we need NaN == NaN
        let nan1 = ScalarValue::Float(f64::NAN);
        let nan2 = ScalarValue::Float(f64::NAN);

        assert_eq!(nan1, nan2);

        // Same bit pattern floats should be equal
        let f1 = ScalarValue::Float(2.5);
        let f2 = ScalarValue::Float(2.5);
        assert_eq!(f1, f2);
    }

    #[test]
    fn test_values_can_be_hashed() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ScalarValue::Int(42));
        set.insert(ScalarValue::Int(42)); // Duplicate
        set.insert(ScalarValue::Int(43));

        assert_eq!(set.len(), 2);

        // Test with floats including NaN
        let mut float_set = HashSet::new();
        float_set.insert(ScalarValue::Float(2.5));
        float_set.insert(ScalarValue::Float(2.5)); // Duplicate
        float_set.insert(ScalarValue::Float(f64::NAN));
        float_set.insert(ScalarValue::Float(f64::NAN)); // Duplicate NaN

        assert_eq!(float_set.len(), 2); // 2.5 and NaN
    }

    #[test]
    fn test_is_type() {
        let int_val = ScalarValue::Int(42);
        assert!(int_val.is_type(&ScalarType::Int));
        assert!(!int_val.is_type(&ScalarType::Float));
    }

    // Bytes tests
    #[test]
    fn test_bytes_equality() {
        let b1 = ScalarValue::Bytes(vec![]);
        let b2 = ScalarValue::Bytes(vec![]);
        let b3 = ScalarValue::Bytes(vec![1, 2, 3]);
        let b4 = ScalarValue::Bytes(vec![1, 2, 3]);

        assert_eq!(b1, b2);
        assert_eq!(b3, b4);
        assert_ne!(b1, b3);
    }

    #[test]
    fn test_bytes_hashing() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ScalarValue::Bytes(vec![1, 2, 3]));
        set.insert(ScalarValue::Bytes(vec![1, 2, 3]));
        set.insert(ScalarValue::Bytes(vec![]));

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_bytes_serialization() {
        let val = ScalarValue::Bytes(vec![1, 2, 3, 255]);
        let serialized = serde_json::to_string(&val).unwrap();
        let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();
        assert_eq!(val, deserialized);
    }

    // Relation value tests (RVAs)
    #[test]
    fn test_relation_value_equality() {
        use crate::tuple;
        use crate::types::{RelationType, TupleType};
        use crate::values::Relation;

        let heading = TupleType::new().with_attribute("a", ScalarType::Int);
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { a: 1i64 }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { a: 1i64 }).unwrap();

        let val1 = ScalarValue::Relation(rel1);
        let val2 = ScalarValue::Relation(rel2);

        assert_eq!(val1, val2);
    }

    #[test]
    fn test_relation_value_hashing() {
        use crate::tuple;
        use crate::types::{RelationType, TupleType};
        use crate::values::Relation;
        use std::collections::HashSet;

        let heading = TupleType::new().with_attribute("a", ScalarType::Int);
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { a: 1i64 }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { a: 1i64 }).unwrap();

        let mut set = HashSet::new();
        set.insert(ScalarValue::Relation(rel1));
        set.insert(ScalarValue::Relation(rel2)); // Duplicate

        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_relation_value_serialization() {
        use crate::tuple;
        use crate::types::{RelationType, TupleType};
        use crate::values::Relation;

        let heading = TupleType::new().with_attribute("a", ScalarType::Int);
        let rel_type = RelationType::new(heading);

        let mut rel = Relation::new(rel_type);
        rel.insert(tuple! { a: 1i64 }).unwrap();
        rel.insert(tuple! { a: 2i64 }).unwrap();

        let val = ScalarValue::Relation(rel);
        let serialized = serde_json::to_string(&val).unwrap();
        let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();

        assert_eq!(val, deserialized);
    }

    #[test]
    fn test_nested_relation_value() {
        use crate::tuple;
        use crate::types::{RelationType, TupleType};
        use crate::values::Relation;

        // Inner relation: {a: Int}
        let inner_heading = TupleType::new().with_attribute("a", ScalarType::Int);
        let inner_rel_type = RelationType::new(inner_heading.clone());

        let mut inner_rel = Relation::new(inner_rel_type.clone());
        inner_rel.insert(tuple! { a: 42i64 }).unwrap();

        // Outer relation: {id: Int, data: Relation}
        let outer_heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("data", ScalarType::Relation(Box::new(inner_rel_type)));

        let outer_rel_type = RelationType::new(outer_heading);
        let mut outer_rel = Relation::new(outer_rel_type);

        outer_rel
            .insert(tuple! {
                id: 1i64,
                data: ScalarValue::Relation(inner_rel)
            })
            .unwrap();

        assert_eq!(outer_rel.cardinality(), 1);

        // Verify we can retrieve the RVA
        let tuple = outer_rel.tuples().next().unwrap();
        let data = tuple.get("data").unwrap();

        match data {
            ScalarValue::Relation(rel) => {
                assert_eq!(rel.cardinality(), 1);
                assert_eq!(rel.degree(), 1);

                let inner_tuple = rel.tuples().next().unwrap();
                assert_eq!(inner_tuple.get("a"), Some(&ScalarValue::Int(42)));
            }
            _ => panic!("Expected relation value"),
        }
    }
}

#[cfg(test)]
mod nan_fix_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_nan_grouping_consistent() {
        let nan1 = ScalarValue::Float(f64::NAN);
        let nan2 = ScalarValue::Float(f64::from_bits(f64::NAN.to_bits() ^ 1));

        assert!(nan1.is_type(&ScalarType::Float));
        assert!(nan2.is_type(&ScalarType::Float));

        // This assertion ensures that different NaN bit patterns are treated as equal
        assert_eq!(nan1, nan2, "All NaNs should be equal");

        let mut set = HashSet::new();
        set.insert(nan1.clone());
        set.insert(nan2.clone());

        assert_eq!(set.len(), 1, "Set should contain only one NaN");
    }
}
