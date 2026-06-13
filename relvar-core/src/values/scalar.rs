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
/// # Examples
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::ScalarType;
    /// use relvar_core::values::ScalarValue;
    ///
    /// let point_type = ScalarType::UserDefined {
    ///     name: "Point".to_string(),
    ///     representation: Box::new(ScalarType::String),
    /// };
    ///
    /// let value = ScalarValue::select(&point_type, ScalarValue::String("1,2".to_string())).unwrap();
    /// ```
    pub fn select(
        type_def: &crate::types::ScalarType,
        value: ScalarValue,
    ) -> Result<ScalarValue, crate::types::scalar::ScalarTypeError> {
        match type_def {
            crate::types::ScalarType::UserDefined { representation, .. } => {
                if !value.is_type(representation) {
                    return Err(crate::types::scalar::ScalarTypeError {
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
                    return Err(crate::types::scalar::ScalarTypeError {
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
    /// # Examples
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
    /// # Examples
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
    /// # Examples
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
mod tests;
