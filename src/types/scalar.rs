use serde::{Deserialize, Serialize};

/// Scalar type representation per Date's relational model.
/// Types are identified by name and support equality comparison.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
}
