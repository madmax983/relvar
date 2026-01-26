use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors related to user-defined type operations
#[derive(Debug, Error)]
pub enum ScalarTypeError {
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },
}

/// Scalar type representation per Date's relational model.
/// Types are identified by name and support equality comparison.
///
/// TTM Prescription 1: The system must allow users to define their own scalar types.
/// This is implemented via the UserDefined variant which supports the POSSREP pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ScalarType {
    /// 64-bit signed integer
    Int,
    /// 64-bit floating point
    Float,
    /// UTF-8 string
    String,
    /// Boolean value
    Bool,
    /// Arbitrary bytes
    Bytes,
    /// Relation-valued attribute type
    Relation(Box<crate::types::RelationType>),
    /// User-defined type with a name and base representation type.
    /// TTM: This implements POSSREP (possible representation) pattern.
    /// The name provides type identity, the representation provides storage.
    UserDefined {
        name: String,
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
