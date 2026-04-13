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
use std::convert::TryFrom;
use thiserror::Error;

/// Errors that can occur during scalar value operations.
///
/// Attempted to use an observer on a built-in type.
///
/// The observer operation is only valid for user-defined types (POSSREP pattern).
/// Built-in types like `Int`, `String`, etc., do not have observers because
/// their representation is their value.
#[derive(Debug, Error)]
#[error("Cannot extract observer from built-in type")]
pub struct ScalarValueError;

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
#[serde(try_from = "ScalarValueUnchecked")]
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
    /// Enables nested relations within tuples, allowing attributes to contain
    /// entire relations as their values (RVA).
    Relation(crate::values::Relation),

    /// User-defined type value.
    ///
    /// Implements the POSSREP pattern: the value carries both its type
    /// identity (via `type_def`) and its representation value (via `value`).
    ///
    /// Use [`ScalarValue::select()`] to create user-defined values.
    UserDefined {
        /// The type definition for this user-defined value.
        type_def: ScalarType,
        /// The underlying representation value.
        value: Box<ScalarValue>,
    },
}

impl ScalarValue {
    /// POSSREP selector: constructs a value of this type from its representation.
    ///
    /// TTM: The selector takes a value of the representation type and produces
    /// a value of this user-defined type.
    pub fn select(
        type_def: &crate::types::ScalarType,
        value: ScalarValue,
    ) -> Result<ScalarValue, crate::types::scalar::ScalarTypeError> {
        match type_def {
            crate::types::ScalarType::UserDefined { representation, .. } => {
                if !value.is_type(representation) {
                    return Err(crate::types::scalar::ScalarTypeError::TypeMismatch {
                        expected: representation.name().to_string(),
                        actual: value.scalar_type().name().to_string(),
                    });
                }
                Ok(ScalarValue::UserDefined {
                    type_def: type_def.clone(),
                    value: Box::new(value),
                })
            }
            ty => {
                if !value.is_type(ty) {
                    return Err(crate::types::scalar::ScalarTypeError::TypeMismatch {
                        expected: ty.name().to_string(),
                        actual: value.scalar_type().name().to_string(),
                    });
                }
                Ok(value)
            }
        }
    }

    /// Resolves the concrete scalar type of this value instance.
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

    /// Checks if this value is of the given type.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::values::ScalarValue;
    /// use relvar_core::types::ScalarType;
    ///
    /// let value = ScalarValue::Int(42);
    /// assert!(value.is_type(&ScalarType::Int));
    /// assert!(!value.is_type(&ScalarType::String));
    /// ```
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
    /// let widget_val = ScalarValue::select(&widget_type, ScalarValue::Int(42)).unwrap();
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
            _ => Err(ScalarValueError),
        }
    }

    /// Helper constructor for user-defined values (used in tests).
    /// Prefer using `ScalarValue::select()` in production code.
    #[cfg(test)]
    pub fn user_defined(type_def: ScalarType, value: ScalarValue) -> Self {
        ScalarValue::UserDefined {
            type_def,
            value: Box::new(value),
        }
    }
}

impl Drop for ScalarValue {
    fn drop(&mut self) {
        if let ScalarValue::UserDefined { value, .. } = self {
            // Iteratively drop nested UserDefined values to prevent stack overflow
            let mut current = std::mem::replace(value, Box::new(ScalarValue::Int(0)));
            while let ScalarValue::UserDefined {
                value: ref mut next,
                ..
            } = *current
            {
                current = std::mem::replace(next, Box::new(ScalarValue::Int(0)));
            }
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
        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return false;
        }

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
            (
                ScalarValue::UserDefined {
                    type_def: type_a,
                    value: val_a,
                },
                ScalarValue::UserDefined {
                    type_def: type_b,
                    value: val_b,
                },
            ) => ScalarValue::eq_user_defined_values(type_a, val_a, type_b, val_b),
            _ => unreachable!("Discriminant check should have caught mismatched types"),
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
            ScalarValue::UserDefined { type_def, value } => {
                ScalarValue::hash_user_defined_value(type_def, value, state);
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

        let cmp_type = type_order(self).cmp(&type_order(other));
        if cmp_type != Ordering::Equal {
            return cmp_type;
        }

        // Same type: order by value
        match (self, other) {
            (ScalarValue::Int(a), ScalarValue::Int(b)) => a.cmp(b),
            (ScalarValue::Float(a), ScalarValue::Float(b)) => Self::cmp_floats(*a, *b),
            (ScalarValue::String(a), ScalarValue::String(b)) => a.cmp(b),
            (ScalarValue::Bool(a), ScalarValue::Bool(b)) => a.cmp(b),
            (ScalarValue::Bytes(a), ScalarValue::Bytes(b)) => a.cmp(b),
            (ScalarValue::Relation(a), ScalarValue::Relation(b)) => Self::cmp_relations(a, b),
            (
                ScalarValue::UserDefined {
                    type_def: type_a,
                    value: val_a,
                },
                ScalarValue::UserDefined {
                    type_def: type_b,
                    value: val_b,
                },
            ) => Self::cmp_user_defined_values(type_a, val_a, type_b, val_b),
            _ => unreachable!("Type orders are equal but types don't match"),
        }
    }
}

impl ScalarValue {
    fn cmp_floats(a: f64, b: f64) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        // For floats, treat all NaNs as equal and greater than any other float
        match (a.is_nan(), b.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                // Correctly order floating point numbers using their bit representation
                // This handles signed zeros (-0.0 < 0.0) and negative numbers correctly
                let a_bits = a.to_bits();
                let b_bits = b.to_bits();
                let a_sign = a_bits >> 63;
                let b_sign = b_bits >> 63;

                if a_sign != b_sign {
                    // Different signs: negative < positive
                    if a_sign == 1 {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                } else {
                    // Same signs
                    if a_sign == 0 {
                        // Both positive: larger magnitude is larger
                        a_bits.cmp(&b_bits)
                    } else {
                        // Both negative: larger magnitude is smaller (more negative)
                        // e.g., -10.0 has larger bit representation than -1.0
                        b_bits.cmp(&a_bits)
                    }
                }
            }
        }
    }

    fn cmp_relations(
        a: &crate::values::Relation,
        b: &crate::values::Relation,
    ) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        // For relations, order by cardinality first, then degree
        match a.cardinality().cmp(&b.cardinality()) {
            Ordering::Equal => a.degree().cmp(&b.degree()),
            other => other,
        }
    }

    fn eq_user_defined_values(
        type_a: &ScalarType,
        val_a: &ScalarValue,
        type_b: &ScalarType,
        val_b: &ScalarValue,
    ) -> bool {
        if type_a != type_b {
            return false;
        }
        // Iterative comparison to prevent stack overflow
        let mut cur_a = val_a;
        let mut cur_b = val_b;
        loop {
            match (cur_a, cur_b) {
                (
                    ScalarValue::UserDefined {
                        type_def: ta,
                        value: va,
                    },
                    ScalarValue::UserDefined {
                        type_def: tb,
                        value: vb,
                    },
                ) => {
                    if ta != tb {
                        return false;
                    }
                    cur_a = va.as_ref();
                    cur_b = vb.as_ref();
                }
                (a, b) => return a == b,
            }
        }
    }

    fn hash_user_defined_value<H: std::hash::Hasher>(
        type_def: &ScalarType,
        value: &ScalarValue,
        state: &mut H,
    ) {
        use std::hash::Hash;
        6u8.hash(state);
        type_def.hash(state);
        // Iterative hash to prevent stack overflow
        let mut cur = value;
        loop {
            match cur {
                ScalarValue::UserDefined {
                    type_def: t,
                    value: v,
                } => {
                    6u8.hash(state);
                    t.hash(state);
                    cur = v.as_ref();
                }
                other => {
                    other.hash(state);
                    break;
                }
            }
        }
    }

    fn cmp_user_defined_values(
        type_a: &ScalarType,
        val_a: &ScalarValue,
        type_b: &ScalarType,
        val_b: &ScalarValue,
    ) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        // Order by type identity first (structural comparison), then by value
        match type_a.cmp(type_b) {
            Ordering::Equal => {
                // Iterative comparison to prevent stack overflow
                let mut cur_a = val_a;
                let mut cur_b = val_b;
                loop {
                    match (cur_a, cur_b) {
                        (
                            ScalarValue::UserDefined {
                                type_def: ta,
                                value: va,
                            },
                            ScalarValue::UserDefined {
                                type_def: tb,
                                value: vb,
                            },
                        ) => match ta.cmp(tb) {
                            Ordering::Equal => {
                                cur_a = va;
                                cur_b = vb;
                            }
                            other => return other,
                        },
                        (a, b) => return a.cmp(b),
                    }
                }
            }
            other => other,
        }
    }
}

// Private structure to assist with deserialization and validation.
// This allows us to intercept deserialization and enforce type consistency and depth limits.
#[derive(Debug, Deserialize)]
enum ScalarValueUnchecked {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Bytes(Vec<u8>),
    Relation(
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
        crate::values::Relation,
    ),
    UserDefined {
        type_def: ScalarType,
        #[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]
        value: Box<ScalarValueUnchecked>,
    },
}

impl TryFrom<ScalarValueUnchecked> for ScalarValue {
    type Error = String;

    fn try_from(unchecked: ScalarValueUnchecked) -> Result<Self, Self::Error> {
        match unchecked {
            ScalarValueUnchecked::Int(v) => Ok(ScalarValue::Int(v)),
            ScalarValueUnchecked::Float(v) => Ok(ScalarValue::Float(v)),
            ScalarValueUnchecked::String(v) => Ok(ScalarValue::String(v)),
            ScalarValueUnchecked::Bool(v) => Ok(ScalarValue::Bool(v)),
            ScalarValueUnchecked::Bytes(v) => Ok(ScalarValue::Bytes(v)),
            ScalarValueUnchecked::Relation(v) => Ok(ScalarValue::Relation(v)),
            ScalarValueUnchecked::UserDefined { type_def, value } => {
                // First, ensure the type definition itself is a UserDefined type.
                // A ScalarValue::UserDefined variant must have a ScalarType::UserDefined type definition.
                // It makes no sense to have ScalarValue::UserDefined { type_def: Int, ... }.
                let representation = match &type_def {
                    ScalarType::UserDefined { representation, .. } => representation,
                    _ => {
                        return Err(format!(
                            "Invalid UserDefined value: type definition must be UserDefined, got {}",
                            type_def.name()
                        ));
                    }
                };

                // Recursively convert and validate the inner value
                let inner_value = ScalarValue::try_from(*value)?;

                // Enforce type consistency: inner value MUST match the representation type
                if !inner_value.is_type(representation) {
                    return Err(format!(
                        "Type mismatch in UserDefined value: type definition expects {}, but value is {}",
                        representation.name(),
                        inner_value.scalar_type().name()
                    ));
                }

                Ok(ScalarValue::UserDefined {
                    type_def,
                    value: Box::new(inner_value),
                })
            }
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
        let widget = ScalarValue::select(&widget_id_type, ScalarValue::Int(42)).unwrap();

        assert_eq!(widget.scalar_type(), widget_id_type);
    }

    #[test]
    fn test_possrep_observer_extracts_representation() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let widget = ScalarValue::select(&widget_id_type, ScalarValue::Int(42)).unwrap();

        // Observer: extract the underlying Int value
        let underlying = widget.observer().unwrap();

        assert_eq!(underlying, ScalarValue::Int(42));
    }

    #[test]
    fn test_nested_user_defined_types() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let special_widget_type =
            ScalarType::user_defined("SpecialWidgetId", widget_id_type.clone());

        let widget = ScalarValue::select(&widget_id_type, ScalarValue::Int(42)).unwrap();
        let special_widget = ScalarValue::select(&special_widget_type, widget.clone()).unwrap();

        assert_ne!(special_widget.scalar_type(), widget_id_type);
        assert_eq!(special_widget.scalar_type(), special_widget_type);
    }

    #[test]
    fn test_type_safety_prevents_wrong_representation() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);

        // Should fail: trying to construct WidgetId from String
        let result = ScalarValue::select(
            &widget_id_type,
            ScalarValue::String("not an int".to_string()),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_user_defined_types_serialize() {
        let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let widget = ScalarValue::select(&widget_id_type, ScalarValue::Int(42)).unwrap();

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
        set.insert(ScalarValue::select(&widget_id_type, ScalarValue::Int(5)).unwrap());
        set.insert(ScalarValue::select(&widget_id_type, ScalarValue::Int(5)).unwrap()); // Duplicate
        set.insert(ScalarValue::select(&supplier_id_type, ScalarValue::Int(5)).unwrap()); // Different type
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

#[cfg(test)]
mod float_ord_tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn test_float_negative_ordering() {
        let neg_ten = ScalarValue::Float(-10.0);
        let neg_one = ScalarValue::Float(-1.0);
        let zero = ScalarValue::Float(0.0);

        assert_eq!(neg_ten.cmp(&neg_one), Ordering::Less);
        assert_eq!(neg_one.cmp(&zero), Ordering::Less);

        // Transitivity
        assert_eq!(neg_ten.cmp(&zero), Ordering::Less);
    }

    #[test]
    fn test_float_signed_zero_ordering() {
        let neg_zero = ScalarValue::Float(-0.0);
        let pos_zero = ScalarValue::Float(0.0);

        assert_eq!(neg_zero.cmp(&pos_zero), Ordering::Less);

        // Verify they are still distinct in Eq (due to bitwise equality)
        assert_ne!(neg_zero, pos_zero);
    }

    #[test]
    fn test_float_mixed_sign_ordering() {
        let neg = ScalarValue::Float(-5.0);
        let pos = ScalarValue::Float(5.0);

        assert_eq!(neg.cmp(&pos), Ordering::Less);
    }

    #[test]
    fn test_nan_ordering() {
        let nan = ScalarValue::Float(f64::NAN);
        let num = ScalarValue::Float(100.0);

        // NaNs should be greater than any number
        assert_eq!(nan.cmp(&num), Ordering::Greater);
        assert_eq!(num.cmp(&nan), Ordering::Less);

        // NaNs equal each other
        let nan2 = ScalarValue::Float(f64::NAN);
        assert_eq!(nan.cmp(&nan2), Ordering::Equal);
    }
}
