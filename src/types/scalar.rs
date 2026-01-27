//! Scalar types for the relational model.
//!
//! This module defines [`ScalarType`], which represents the atomic types that
//! can appear as attribute values in tuples. It includes built-in types
//! (Int, Float, String, Bool, Bytes) and user-defined types.
//!
//! # TTM Compliance
//!
//! - **Prescription 1**: User-defined scalar types are supported via the POSSREP
//!   (possible representation) pattern
//! - Type identity is determined by name, not representation
//! - Built-in types provide the foundation for user-defined types
//!
//! # Example
//!
//! ```
//! use relvar::types::ScalarType;
//!
//! // Built-in types
//! let int_type = ScalarType::Int;
//! let string_type = ScalarType::String;
//!
//! // User-defined types with distinct identity
//! let widget_id = ScalarType::user_defined("WidgetId", ScalarType::Int);
//! let supplier_id = ScalarType::user_defined("SupplierId", ScalarType::Int);
//!
//! // Same representation, but different types!
//! assert_ne!(widget_id, supplier_id);
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during scalar type operations.
#[derive(Debug, Error)]
pub enum ScalarTypeError {
    /// A value's type doesn't match the expected type.
    ///
    /// This typically occurs when using a selector with a value of the
    /// wrong representation type.
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch {
        /// The expected type name.
        expected: String,
        /// The actual type name of the provided value.
        actual: String,
    },
}

/// Represents a scalar (atomic) type in the relational model.
///
/// Scalar types are the building blocks of the type system. Each attribute
/// in a tuple has a scalar type that defines what values it can hold.
///
/// # Built-in Types
///
/// - [`Int`](ScalarType::Int) - 64-bit signed integer
/// - [`Float`](ScalarType::Float) - 64-bit floating point
/// - [`String`](ScalarType::String) - UTF-8 string
/// - [`Bool`](ScalarType::Bool) - Boolean (true/false)
/// - [`Bytes`](ScalarType::Bytes) - Arbitrary byte sequence
///
/// # Advanced Types
///
/// - [`Relation`](ScalarType::Relation) - Nested relation (relation-valued attribute)
/// - [`UserDefined`](ScalarType::UserDefined) - Custom type with POSSREP pattern
///
/// # TTM Prescription 1
///
/// Users can define their own scalar types using the POSSREP (possible
/// representation) pattern. A user-defined type has:
///
/// - A **name** that provides type identity
/// - A **representation** type for storage
///
/// Two user-defined types with the same representation but different names
/// are considered distinct types.
///
/// # Example
///
/// ```
/// use relvar::types::ScalarType;
///
/// // Built-in types
/// let age_type = ScalarType::Int;
/// let name_type = ScalarType::String;
///
/// // User-defined type for type safety
/// let employee_id = ScalarType::user_defined("EmployeeId", ScalarType::Int);
/// let department_id = ScalarType::user_defined("DepartmentId", ScalarType::Int);
///
/// // These are different types despite same representation
/// assert_ne!(employee_id, department_id);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalarType {
    /// 64-bit signed integer.
    ///
    /// Corresponds to Rust's `i64` type.
    Int,

    /// 64-bit floating point number.
    ///
    /// Corresponds to Rust's `f64` type.
    Float,

    /// UTF-8 encoded string.
    ///
    /// Corresponds to Rust's `String` type.
    String,

    /// Boolean value (true or false).
    ///
    /// Corresponds to Rust's `bool` type.
    Bool,

    /// Arbitrary byte sequence.
    ///
    /// Corresponds to Rust's `Vec<u8>` type.
    Bytes,

    /// Relation-valued attribute (RVA) type.
    ///
    /// Contains a nested relation, enabling hierarchical data modeling.
    /// The boxed `RelationType` specifies the heading of the nested relation.
    Relation(Box<crate::types::RelationType>),

    /// User-defined scalar type.
    ///
    /// Implements the POSSREP (possible representation) pattern from TTM.
    /// The type has a unique name (providing type identity) and a base
    /// representation type (defining storage and valid values).
    ///
    /// # Example
    ///
    /// ```
    /// use relvar::types::ScalarType;
    ///
    /// let currency = ScalarType::user_defined("Currency", ScalarType::Float);
    /// assert_eq!(currency.name(), "Currency");
    /// ```
    UserDefined {
        /// The unique name identifying this type.
        name: String,
        /// The underlying representation type.
        representation: Box<ScalarType>,
    },
}

impl ScalarType {
    /// Returns the name of the type
    pub fn name(&self) -> String {
        match self {
            ScalarType::Int => "Int".to_string(),
            ScalarType::Float => "Float".to_string(),
            ScalarType::String => "String".to_string(),
            ScalarType::Bool => "Bool".to_string(),
            ScalarType::Bytes => "Bytes".to_string(),
            ScalarType::Relation(_) => "Relation".to_string(),
            ScalarType::UserDefined { name, .. } => name.clone(),
        }
    }

    /// Creates a user-defined type with the given name and representation type.
    ///
    /// TTM Prescription 1: Users can define their own scalar types.
    /// TTM: This implements the POSSREP pattern - the type has a name (identity)
    /// and a representation type (storage).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::types::ScalarType;
    ///
    /// let widget_id = ScalarType::user_defined("WidgetId", ScalarType::Int);
    /// let supplier_id = ScalarType::user_defined("SupplierId", ScalarType::Int);
    ///
    /// // Same representation, but different types
    /// assert_ne!(widget_id, supplier_id);
    /// ```
    pub fn user_defined(name: impl Into<String>, representation: ScalarType) -> Self {
        ScalarType::UserDefined {
            name: name.into(),
            representation: Box::new(representation),
        }
    }

    /// POSSREP selector: constructs a value of this type from its representation.
    ///
    /// TTM: The selector takes a value of the representation type and produces
    /// a value of this user-defined type.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the provided value's type doesn't match the expected
    /// representation type.
    pub fn selector(
        &self,
        value: crate::values::ScalarValue,
    ) -> Result<crate::values::ScalarValue, ScalarTypeError> {
        use crate::values::ScalarValue;

        match self {
            ScalarType::UserDefined { representation, .. } => {
                // Check that the value matches the representation type
                if !value.is_type(representation) {
                    return Err(ScalarTypeError::TypeMismatch {
                        expected: representation.name(),
                        actual: value.scalar_type().name(),
                    });
                }
                Ok(ScalarValue::UserDefined {
                    type_def: self.clone(),
                    value: Box::new(value),
                })
            }
            // For built-in types, the value must already be of this type
            ty => {
                if !value.is_type(ty) {
                    return Err(ScalarTypeError::TypeMismatch {
                        expected: ty.name(),
                        actual: value.scalar_type().name(),
                    });
                }
                Ok(value)
            }
        }
    }
}

impl std::hash::Hash for ScalarType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the discriminant first
        std::mem::discriminant(self).hash(state);

        // Then hash the data based on variant
        match self {
            ScalarType::Int => {}
            ScalarType::Float => {}
            ScalarType::String => {}
            ScalarType::Bool => {}
            ScalarType::Bytes => {}
            ScalarType::Relation(rel_type) => {
                // Hash the relation type's heading
                rel_type.heading().hash(state);
            }
            ScalarType::UserDefined {
                name,
                representation,
            } => {
                name.hash(state);
                representation.hash(state);
            }
        }
    }
}

impl PartialOrd for ScalarType {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScalarType {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        // Helper to get a discriminant value for ordering
        let disc_value = |t: &ScalarType| match t {
            ScalarType::Int => 0,
            ScalarType::Float => 1,
            ScalarType::String => 2,
            ScalarType::Bool => 3,
            ScalarType::Bytes => 4,
            ScalarType::Relation(_) => 5,
            ScalarType::UserDefined { .. } => 6,
        };

        match disc_value(self).cmp(&disc_value(other)) {
            Ordering::Equal => {
                // Same variant, compare data
                match (self, other) {
                    (ScalarType::Int, ScalarType::Int)
                    | (ScalarType::Float, ScalarType::Float)
                    | (ScalarType::String, ScalarType::String)
                    | (ScalarType::Bool, ScalarType::Bool)
                    | (ScalarType::Bytes, ScalarType::Bytes) => Ordering::Equal,
                    (ScalarType::Relation(a), ScalarType::Relation(b)) => {
                        // Compare relation types by their headings
                        // Since TupleType doesn't have Ord, we need a custom comparison
                        let a_attrs: Vec<_> = a
                            .heading()
                            .attribute_names()
                            .map(|n| (n, a.heading().get_attribute_type(n).unwrap()))
                            .collect();
                        let b_attrs: Vec<_> = b
                            .heading()
                            .attribute_names()
                            .map(|n| (n, b.heading().get_attribute_type(n).unwrap()))
                            .collect();

                        // Compare by count first, then by sorted attributes
                        match a_attrs.len().cmp(&b_attrs.len()) {
                            Ordering::Equal => {
                                let mut a_sorted = a_attrs;
                                let mut b_sorted = b_attrs;
                                a_sorted.sort_by_key(|(name, _)| *name);
                                b_sorted.sort_by_key(|(name, _)| *name);

                                for ((a_name, a_ty), (b_name, b_ty)) in
                                    a_sorted.iter().zip(b_sorted.iter())
                                {
                                    match a_name.cmp(b_name) {
                                        Ordering::Equal => match a_ty.cmp(b_ty) {
                                            Ordering::Equal => continue,
                                            other => return other,
                                        },
                                        other => return other,
                                    }
                                }
                                Ordering::Equal
                            }
                            other => other,
                        }
                    }
                    (
                        ScalarType::UserDefined {
                            name: a_name,
                            representation: a_rep,
                        },
                        ScalarType::UserDefined {
                            name: b_name,
                            representation: b_rep,
                        },
                    ) => match a_name.cmp(b_name) {
                        Ordering::Equal => a_rep.cmp(b_rep),
                        other => other,
                    },
                    _ => unreachable!("Discriminants matched but variants don't"),
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
    fn test_scalar_types_can_be_compared_for_equality() {
        let int1 = ScalarType::Int;
        let int2 = ScalarType::Int;
        let float1 = ScalarType::Float;

        assert_eq!(int1, int2);
        assert_ne!(int1, float1);
    }

    #[test]
    fn test_type_names_are_unique_identifiers() {
        let int_type = ScalarType::Int;
        let float_type = ScalarType::Float;
        let string_type = ScalarType::String;
        let bool_type = ScalarType::Bool;
        let bytes_type = ScalarType::Bytes;

        assert_eq!(int_type.name(), "Int");
        assert_eq!(float_type.name(), "Float");
        assert_eq!(string_type.name(), "String");
        assert_eq!(bool_type.name(), "Bool");
        assert_eq!(bytes_type.name(), "Bytes");

        // All names should be unique
        let names = [
            int_type.name(),
            float_type.name(),
            string_type.name(),
            bool_type.name(),
            bytes_type.name(),
        ];
        let unique_names: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique_names.len());
    }

    #[test]
    fn test_built_in_types_exist() {
        // Verify all built-in types can be constructed
        let _ = ScalarType::Int;
        let _ = ScalarType::Float;
        let _ = ScalarType::String;
        let _ = ScalarType::Bool;
        let _ = ScalarType::Bytes;
    }

    #[test]
    fn test_scalar_types_implement_clone() {
        let int_type = ScalarType::Int;
        let cloned = int_type.clone();
        assert_eq!(int_type, cloned);
    }

    #[test]
    fn test_scalar_types_can_be_hashed() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ScalarType::Int);
        set.insert(ScalarType::Float);
        set.insert(ScalarType::Int); // Duplicate

        assert_eq!(set.len(), 2); // Only Int and Float
        assert!(set.contains(&ScalarType::Int));
        assert!(set.contains(&ScalarType::Float));
    }

    #[test]
    fn test_builtin_selector_validates_type() {
        use crate::values::ScalarValue;

        // Selector for built-in types should reject values of wrong type
        let int_type = ScalarType::Int;
        let string_value = ScalarValue::String("not an int".to_string());

        let result = int_type.selector(string_value);
        assert!(result.is_err());

        // Should accept correct type
        let int_value = ScalarValue::Int(42);
        let result = int_type.selector(int_value.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), int_value);
    }
}
