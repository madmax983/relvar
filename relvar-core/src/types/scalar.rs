//! Scalar types for the relational model.
//!
//! This module defines [`ScalarType`], which represents the atomic types that
//! can appear as attribute values in tuples. It includes built-in types
//! (Int, Float, String, Bool, Bytes).
//!
//! # Example
//!
//! ```
//! use relvar_core::types::ScalarType;
//!
//! // Built-in types
//! let int_type = ScalarType::Int;
//! let string_type = ScalarType::String;
//! ```

use serde::{Deserialize, Serialize};

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

    // Tests for Ord/PartialOrd implementations (for coverage)
    #[test]
    fn test_scalar_type_ord_basic_types() {
        use std::cmp::Ordering;

        // Test discriminant ordering: Int < Float < String < Bool < Bytes < Relation
        assert_eq!(ScalarType::Int.cmp(&ScalarType::Float), Ordering::Less);
        assert_eq!(ScalarType::Float.cmp(&ScalarType::String), Ordering::Less);
        assert_eq!(ScalarType::String.cmp(&ScalarType::Bool), Ordering::Less);
        assert_eq!(ScalarType::Bool.cmp(&ScalarType::Bytes), Ordering::Less);

        // Same types are equal
        assert_eq!(ScalarType::Int.cmp(&ScalarType::Int), Ordering::Equal);
        assert_eq!(ScalarType::Float.cmp(&ScalarType::Float), Ordering::Equal);
        assert_eq!(ScalarType::String.cmp(&ScalarType::String), Ordering::Equal);
        assert_eq!(ScalarType::Bool.cmp(&ScalarType::Bool), Ordering::Equal);
        assert_eq!(ScalarType::Bytes.cmp(&ScalarType::Bytes), Ordering::Equal);
    }

    #[test]
    fn test_scalar_type_ord_relation_types() {
        use crate::types::{RelationType, TupleType};
        use std::cmp::Ordering;

        // Create relation types with different headings
        let heading_a = TupleType::new().with_attribute("a", ScalarType::Int);
        let heading_b = TupleType::new().with_attribute("b", ScalarType::Int);
        let heading_a_copy = TupleType::new().with_attribute("a", ScalarType::Int);

        let rel_type_a = ScalarType::Relation(Box::new(RelationType::new(heading_a)));
        let rel_type_b = ScalarType::Relation(Box::new(RelationType::new(heading_b)));
        let rel_type_a_copy = ScalarType::Relation(Box::new(RelationType::new(heading_a_copy)));

        // Same heading should be equal
        assert_eq!(rel_type_a.cmp(&rel_type_a_copy), Ordering::Equal);

        // Different headings should have consistent ordering
        let result = rel_type_a.cmp(&rel_type_b);
        assert_ne!(result, Ordering::Equal);

        // Ordering should be transitive and antisymmetric
        assert_eq!(rel_type_b.cmp(&rel_type_a), result.reverse());
    }

    #[test]
    fn test_scalar_type_ord_relation_types_different_degrees() {
        use crate::types::{RelationType, TupleType};
        use std::cmp::Ordering;

        // Different degree headings
        let heading_1 = TupleType::new().with_attribute("a", ScalarType::Int);
        let heading_2 = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::Int);

        let rel_type_1 = ScalarType::Relation(Box::new(RelationType::new(heading_1)));
        let rel_type_2 = ScalarType::Relation(Box::new(RelationType::new(heading_2)));

        // Different degrees should have consistent ordering
        let result = rel_type_1.cmp(&rel_type_2);
        assert_ne!(result, Ordering::Equal);
        assert_eq!(rel_type_2.cmp(&rel_type_1), result.reverse());
    }

    #[test]
    fn test_scalar_type_partial_ord_consistency() {
        // PartialOrd should be consistent with Ord
        let int_type = ScalarType::Int;
        let float_type = ScalarType::Float;

        assert_eq!(
            int_type.partial_cmp(&float_type),
            Some(int_type.cmp(&float_type))
        );
        assert_eq!(
            int_type.partial_cmp(&int_type),
            Some(std::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn test_scalar_type_ord_reflexivity() {
        // x.cmp(x) == Equal (reflexivity)
        let types = vec![
            ScalarType::Int,
            ScalarType::Float,
            ScalarType::String,
            ScalarType::Bool,
            ScalarType::Bytes,
        ];

        for ty in types {
            assert_eq!(ty.cmp(&ty), std::cmp::Ordering::Equal);
        }
    }

    #[test]
    fn test_scalar_type_ord_transitivity() {
        use std::cmp::Ordering;

        // If a < b and b < c, then a < c (transitivity)
        let a = ScalarType::Int;
        let b = ScalarType::Float;
        let c = ScalarType::String;

        assert_eq!(a.cmp(&b), Ordering::Less);
        assert_eq!(b.cmp(&c), Ordering::Less);
        assert_eq!(a.cmp(&c), Ordering::Less);
    }

    #[test]
    fn test_scalar_type_can_be_sorted() {
        // Practical test: should be able to sort a Vec of ScalarTypes
        let mut types = [
            ScalarType::String,
            ScalarType::Int,
            ScalarType::Float,
            ScalarType::Bool,
            ScalarType::Bytes,
        ];

        types.sort();

        // Should be sorted by discriminant order
        assert_eq!(types[0], ScalarType::Int);
        assert_eq!(types[1], ScalarType::Float);
        assert_eq!(types[2], ScalarType::String);
        assert_eq!(types[3], ScalarType::Bool);
        assert_eq!(types[4], ScalarType::Bytes);
    }
}
