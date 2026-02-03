//! Scalar values for the relational model.
//!
//! This module defines [`ScalarValue`], which represents atomic runtime values
//! that can be stored as attribute values in tuples.
//!
//! # TTM Compliance
//!
//! - All values carry their type (introspection is possible)
//! - No NULL values are permitted
//! - User-defined values via POSSREP pattern (Prescription 1)
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
use thiserror::Error;

/// Errors that can occur during scalar value operations.
#[derive(Debug, Error)]
pub enum ScalarValueError {
    /// Attempted to use an observer on a built-in type.
    ///
    /// The observer operation is only valid for user-defined types.
    #[error("Cannot extract observer from built-in type")]
    NotUserDefined,
}

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
/// - [`UserDefined`](ScalarValue::UserDefined) - Custom type value
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

    /// User-defined type value.
    ///
    /// Implements the POSSREP pattern: the value carries both its type
    /// identity (via `type_def`) and its representation value (via `value`).
    ///
    /// Use [`ScalarType::selector()`] to create user-defined values.
    UserDefined {
        /// The type definition for this user-defined value.
        type_def: ScalarType,
        /// The underlying representation value.
        value: Box<ScalarValue>,
    },
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
            ScalarValue::UserDefined { type_def, .. } => type_def.clone(),
        }
    }

    /// Checks if this value is of the given type
    pub fn is_type(&self, ty: &ScalarType) -> bool {
        &self.scalar_type() == ty
    }

    /// POSSREP observer: extracts the underlying representation value.
    ///
    /// TTM: The observer function extracts the representation from a
    /// user-defined type value.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::ScalarType;
    /// use relvar_core::values::ScalarValue;
    ///
    /// let widget_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    /// let widget_val = widget_type.selector(ScalarValue::Int(42)).unwrap();
    ///
    /// // Observer extracts the Int(42)
    /// let representation = widget_val.observer().unwrap();
    /// assert_eq!(representation, ScalarValue::Int(42));
    ///
    /// // Built-in types have no observer
    /// let raw_int = ScalarValue::Int(42);
    /// assert!(raw_int.observer().is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Err` if called on a built-in type (only user-defined types
    /// have observers).
    pub fn observer(&self) -> Result<ScalarValue, ScalarValueError> {
        match self {
            ScalarValue::UserDefined { value, .. } => Ok((**value).clone()),
            _ => Err(ScalarValueError::NotUserDefined),
        }
    }

    /// Helper constructor for user-defined values (used in tests).
    /// Prefer using `ScalarType::selector()` in production code.
    #[cfg(test)]
    pub fn user_defined(type_def: ScalarType, value: ScalarValue) -> Self {
        ScalarValue::UserDefined {
            type_def,
            value: Box::new(value),
        }
    }
}

// Custom PartialEq implementation for ScalarValue
// Note: Float comparison uses bit equality, which is appropriate for
// database values (we want NaN == NaN for set semantics)
//
// TTM: User-defined values are equal only if they have the same type AND
// the same representation value. This ensures type safety.
impl PartialEq for ScalarValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ScalarValue::Int(a), ScalarValue::Int(b)) => a == b,
            (ScalarValue::Float(a), ScalarValue::Float(b)) => a.to_bits() == b.to_bits(),
            (ScalarValue::String(a), ScalarValue::String(b)) => a == b,
            (ScalarValue::Bool(a), ScalarValue::Bool(b)) => a == b,
            (ScalarValue::Bytes(a), ScalarValue::Bytes(b)) => a == b,
            (ScalarValue::Relation(a), ScalarValue::Relation(b)) => a == b,
            (
                ScalarValue::UserDefined {
                    type_def: type_a,
                    value: val_a,
                },
                ScalarValue::UserDefined {
                    type_def: type_b,
                    value: val_b,
                },
            ) => type_a == type_b && val_a == val_b,
            _ => false,
        }
    }
}

// Custom Eq implementation for ScalarValue
impl Eq for ScalarValue {}

// Custom Hash implementation
// TTM: User-defined values hash based on both type and value
impl std::hash::Hash for ScalarValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ScalarValue::Int(v) => {
                0u8.hash(state);
                v.hash(state);
            }
            ScalarValue::Float(v) => {
                1u8.hash(state);
                v.to_bits().hash(state);
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
            ScalarValue::UserDefined { type_def, value } => {
                6u8.hash(state);
                type_def.hash(state);
                value.hash(state);
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
// TTM: User-defined values are ordered by type identity first, then by representation value
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
                ScalarValue::UserDefined { .. } => 6,
            }
        }

        // Compare types first
        match type_order(self).cmp(&type_order(other)) {
            Ordering::Equal => {
                // Same type: order by value
                match (self, other) {
                    (ScalarValue::Int(a), ScalarValue::Int(b)) => a.cmp(b),
                    (ScalarValue::Float(a), ScalarValue::Float(b)) => {
                        // For floats, use bit ordering (treats NaN consistently)
                        a.to_bits().cmp(&b.to_bits())
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
                    (
                        ScalarValue::UserDefined {
                            type_def: type_a,
                            value: val_a,
                        },
                        ScalarValue::UserDefined {
                            type_def: type_b,
                            value: val_b,
                        },
                    ) => {
                        // Order by type identity first (structural comparison), then by value
                        match type_a.cmp(type_b) {
                            Ordering::Equal => val_a.cmp(val_b),
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

    // Consolidated tests from user_defined_test.rs

    #[test]
    fn test_user_defined_types_are_distinct_from_builtin_types() {
        // Define two user-defined types, both backed by Int
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let supplier_id_type = ScalarType::user_defined("SupplierId", ScalarType::Int);

        // These types should NOT be equal even though they have the same representation
        assert_ne!(widget_id_type, supplier_id_type);

        // They should also not equal the built-in Int type
        assert_ne!(widget_id_type, ScalarType::Int);
        assert_ne!(supplier_id_type, ScalarType::Int);
    }

    #[test]
    fn test_user_defined_values_are_type_safe() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let supplier_id_type = ScalarType::user_defined("SupplierId", ScalarType::Int);

        // Create values with the same underlying Int value (5)
        let widget_5 = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));
        let supplier_5 = ScalarValue::user_defined(supplier_id_type.clone(), ScalarValue::Int(5));

        // These should NOT be equal - different types!
        assert_ne!(widget_5, supplier_5);

        // Values should also not equal raw Int(5)
        assert_ne!(widget_5, ScalarValue::Int(5));
        assert_ne!(supplier_5, ScalarValue::Int(5));
    }

    #[test]
    fn test_user_defined_values_of_same_type_are_equal() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

        let widget_5_a = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));
        let widget_5_b = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));
        let widget_7 = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(7));

        assert_eq!(widget_5_a, widget_5_b);
        assert_ne!(widget_5_a, widget_7);
    }

    #[test]
    fn test_user_defined_types_have_names() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

        assert_eq!(widget_id_type.name(), "WidgetId");
    }

    #[test]
    fn test_user_defined_values_carry_their_type() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let widget_5 = ScalarValue::user_defined(widget_id_type.clone(), ScalarValue::Int(5));

        assert_eq!(widget_5.scalar_type(), widget_id_type);
        assert_ne!(widget_5.scalar_type(), ScalarType::Int);
    }

    #[test]
    fn test_possrep_selector_constructs_value() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

        // Selector: construct a WidgetId from an Int
        let widget = widget_id_type.selector(ScalarValue::Int(42)).unwrap();

        assert_eq!(widget.scalar_type(), widget_id_type);
    }

    #[test]
    fn test_possrep_observer_extracts_representation() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let widget = widget_id_type.selector(ScalarValue::Int(42)).unwrap();

        // Observer: extract the underlying Int value
        let underlying = widget.observer().unwrap();

        assert_eq!(underlying, ScalarValue::Int(42));
    }

    #[test]
    fn test_nested_user_defined_types() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let special_widget_type =
            ScalarType::user_defined("SpecialWidgetId", widget_id_type.clone());

        let widget = widget_id_type.selector(ScalarValue::Int(42)).unwrap();
        let special_widget = special_widget_type.selector(widget.clone()).unwrap();

        assert_ne!(special_widget.scalar_type(), widget_id_type);
        assert_eq!(special_widget.scalar_type(), special_widget_type);
    }

    #[test]
    fn test_type_safety_prevents_wrong_representation() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

        // Should fail: trying to construct WidgetId from String
        let result = widget_id_type.selector(ScalarValue::String("not an int".to_string()));

        assert!(result.is_err());
    }

    #[test]
    fn test_user_defined_types_serialize() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let widget = widget_id_type.selector(ScalarValue::Int(42)).unwrap();

        let serialized = serde_json::to_string(&widget).unwrap();
        let deserialized: ScalarValue = serde_json::from_str(&serialized).unwrap();

        assert_eq!(widget, deserialized);
        assert_eq!(deserialized.scalar_type(), widget_id_type);
    }

    #[test]
    fn test_user_defined_values_can_be_hashed() {
        use std::collections::HashSet;

        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let supplier_id_type = ScalarType::user_defined("SupplierId", ScalarType::Int);

        let mut set = HashSet::new();
        set.insert(widget_id_type.selector(ScalarValue::Int(5)).unwrap());
        set.insert(widget_id_type.selector(ScalarValue::Int(5)).unwrap()); // Duplicate
        set.insert(supplier_id_type.selector(ScalarValue::Int(5)).unwrap()); // Different type
        set.insert(ScalarValue::Int(5)); // Raw Int

        // Should have 3 distinct values:
        // - WidgetId(5)
        // - SupplierId(5)
        // - Int(5)
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_user_defined_type_names_must_be_unique() {
        let type1 = ScalarType::user_defined("MyType", ScalarType::Int);
        let type2 = ScalarType::user_defined("MyType", ScalarType::String);

        // This test documents that types with the same name but different
        // representations are distinct, as `PartialEq` is structural.
        assert_ne!(type1, type2);
        assert_eq!(type1.name(), type2.name());
    }
}
