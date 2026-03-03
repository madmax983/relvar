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
//! - Type identity for user-defined types is structural (name + representation)
//! - Built-in types provide the foundation for user-defined types
//!
//! # Example
//!
//! ```
//! use relvar_core::types::ScalarType;
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

use crate::utils::recursion::DepthGuarded;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
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
/// use relvar_core::types::ScalarType;
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
#[serde(try_from = "ScalarTypeUnchecked")]
pub enum ScalarType {
    /// 64-bit signed integer.
    ///
    /// Corresponds to Rust's `i64` type.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::values::ScalarValue;
    /// let val = ScalarValue::Int(42);
    /// ```
    Int,

    /// 64-bit floating point number.
    ///
    /// Corresponds to Rust's `f64` type.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::values::ScalarValue;
    /// let val = ScalarValue::Float(3.14);
    /// ```
    Float,

    /// UTF-8 encoded string.
    ///
    /// Corresponds to Rust's `String` type.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::values::ScalarValue;
    /// let val = ScalarValue::String("Relvar".to_string());
    /// ```
    String,

    /// Boolean value (true or false).
    ///
    /// Corresponds to Rust's `bool` type.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::values::ScalarValue;
    /// let val = ScalarValue::Bool(true);
    /// ```
    Bool,

    /// Arbitrary byte sequence.
    ///
    /// Corresponds to Rust's `Vec<u8>` type.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::values::ScalarValue;
    /// let val = ScalarValue::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]);
    /// ```
    Bytes,

    /// Relation-valued attribute (RVA) type.
    ///
    /// Contains a nested relation, enabling hierarchical data modeling.
    /// The boxed `RelationType` specifies the heading of the nested relation.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{ScalarType, TupleType, RelationType};
    ///
    /// // Define the type for items in an order
    /// let item_type = TupleType::new()
    ///     .with_attribute("product_id", ScalarType::Int)
    ///     .with_attribute("quantity", ScalarType::Int);
    ///
    /// // Define the type for the RVA "items"
    /// let items_rva_type = ScalarType::Relation(Box::new(RelationType::new(item_type)));
    ///
    /// // Define the order type containing the RVA
    /// let order_type = TupleType::new()
    ///     .with_attribute("order_id", ScalarType::Int)
    ///     .with_attribute("items", items_rva_type);
    /// ```
    Relation(Box<crate::types::RelationType>),

    /// User-defined scalar type.
    ///
    /// Implements the POSSREP (possible representation) pattern from TTM.
    /// The type has a unique name (providing type identity) and a base
    /// representation type (defining storage and valid values).
    ///
    /// # TTM Prescription 1
    ///
    /// User-defined types provide strong typing. A value of type `EmployeeId`
    /// is distinct from `DepartmentId`, even if both are represented by `Int`.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::ScalarType;
    /// use relvar_core::values::ScalarValue;
    ///
    /// // Define distinct types for IDs
    /// let emp_id_type = ScalarType::user_defined("EmployeeId", ScalarType::Int);
    /// let dept_id_type = ScalarType::user_defined("DepartmentId", ScalarType::Int);
    ///
    /// // They are not equal
    /// assert_ne!(emp_id_type, dept_id_type);
    ///
    /// // Creating values requires the selector
    /// let id_val = emp_id_type.selector(ScalarValue::Int(100)).unwrap();
    /// assert_eq!(id_val.scalar_type().name(), "EmployeeId");
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

    /// Returns the nesting depth of this type.
    ///
    /// - Primitive types have depth 1.
    /// - Recursive types (UserDefined, Relation) have 1 + depth of inner type.
    ///
    /// This is used to enforce `MAX_TYPE_DEPTH` to prevent stack overflow.
    pub fn depth(&self) -> usize {
        match self {
            ScalarType::Relation(rel_type) => 1 + rel_type.depth(),
            ScalarType::UserDefined { representation, .. } => 1 + representation.depth(),
            _ => 1,
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
    /// use relvar_core::types::ScalarType;
    ///
    /// let widget_id = ScalarType::user_defined("WidgetId", ScalarType::Int);
    /// let supplier_id = ScalarType::user_defined("SupplierId", ScalarType::Int);
    ///
    /// // Same representation, but different types
    /// assert_ne!(widget_id, supplier_id);
    /// ```
    pub fn user_defined(name: impl Into<String>, representation: ScalarType) -> Self {
        if representation.depth() + 1 > crate::types::MAX_TYPE_DEPTH {
            panic!(
                "Type nesting too deep: {} (limit: {})",
                representation.depth() + 1,
                crate::types::MAX_TYPE_DEPTH
            );
        }
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
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::ScalarType;
    /// use relvar_core::values::ScalarValue;
    ///
    /// // Define a user-defined type backed by Int
    /// let widget_id_type = ScalarType::user_defined("WidgetId", ScalarType::Int);
    ///
    /// // Successful selection
    /// let widget = widget_id_type.selector(ScalarValue::Int(42)).unwrap();
    /// assert_eq!(widget.scalar_type().name(), "WidgetId");
    ///
    /// // Type mismatch error
    /// let result = widget_id_type.selector(ScalarValue::String("not an int".into()));
    /// assert!(result.is_err());
    /// ```
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
                        let a_attrs: Vec<_> = a.heading().attributes().iter().collect();
                        let b_attrs: Vec<_> = b.heading().attributes().iter().collect();

                        // Compare by count first, then by sorted attributes
                        match a_attrs.len().cmp(&b_attrs.len()) {
                            Ordering::Equal => {
                                // BTreeMap iterators are already sorted by key, so we don't need
                                // to sort again.
                                for ((a_name, a_ty), (b_name, b_ty)) in
                                    a_attrs.iter().zip(b_attrs.iter())
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

    // Tests for Ord/PartialOrd implementations (for coverage)
    #[test]
    fn test_scalar_type_ord_basic_types() {
        use std::cmp::Ordering;

        // Test discriminant ordering: Int < Float < String < Bool < Bytes < Relation < UserDefined
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
    fn test_scalar_type_ord_user_defined_by_name() {
        use std::cmp::Ordering;

        let widget_id = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let supplier_id = ScalarType::user_defined("SupplierId", ScalarType::Int);
        let widget_id_copy = ScalarType::user_defined("WidgetId", ScalarType::Int);

        // Same name and representation should be equal
        assert_eq!(widget_id.cmp(&widget_id_copy), Ordering::Equal);

        // Different names should have consistent ordering
        let result = widget_id.cmp(&supplier_id);
        assert_ne!(result, Ordering::Equal);
        assert_eq!(supplier_id.cmp(&widget_id), result.reverse());
    }

    #[test]
    fn test_scalar_type_ord_user_defined_by_representation() {
        use std::cmp::Ordering;

        // Same name but different representation
        let widget_id_int = ScalarType::user_defined("WidgetId", ScalarType::Int);
        let widget_id_string = ScalarType::user_defined("WidgetId", ScalarType::String);

        // Different representations should have consistent ordering
        let result = widget_id_int.cmp(&widget_id_string);
        assert_ne!(result, Ordering::Equal);
        assert_eq!(widget_id_string.cmp(&widget_id_int), result.reverse());
    }

    #[test]
    fn test_scalar_type_ord_mixed_variants() {
        use crate::types::{RelationType, TupleType};
        use std::cmp::Ordering;

        let int_type = ScalarType::Int;
        let relation_type = ScalarType::Relation(Box::new(RelationType::new(
            TupleType::new().with_attribute("a", ScalarType::Int),
        )));
        let user_type = ScalarType::user_defined("CustomType", ScalarType::Int);

        // Relation comes after basic types
        assert_eq!(int_type.cmp(&relation_type), Ordering::Less);

        // UserDefined comes after Relation
        assert_eq!(relation_type.cmp(&user_type), Ordering::Less);

        // Transitive property
        assert_eq!(int_type.cmp(&user_type), Ordering::Less);
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
            ScalarType::user_defined("Test", ScalarType::Int),
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

// Private structure to assist with deserialization and validation.
// This allows us to intercept deserialization and enforce invariants (MAX_TYPE_DEPTH)
// that could be bypassed by serde if we derived Deserialize directly on ScalarType.
#[derive(Debug, Deserialize)]
enum ScalarTypeUnchecked {
    Int,
    Float,
    String,
    Bool,
    Bytes,
    Relation(DepthGuarded<Box<crate::types::RelationType>>),
    UserDefined {
        name: String,
        representation: Box<DepthGuarded<ScalarTypeUnchecked>>,
    },
}

impl TryFrom<ScalarTypeUnchecked> for ScalarType {
    type Error = String;

    fn try_from(unchecked: ScalarTypeUnchecked) -> Result<Self, Self::Error> {
        let ty = match unchecked {
            ScalarTypeUnchecked::Int => ScalarType::Int,
            ScalarTypeUnchecked::Float => ScalarType::Float,
            ScalarTypeUnchecked::String => ScalarType::String,
            ScalarTypeUnchecked::Bool => ScalarType::Bool,
            ScalarTypeUnchecked::Bytes => ScalarType::Bytes,
            ScalarTypeUnchecked::Relation(rel) => ScalarType::Relation(rel.0),
            ScalarTypeUnchecked::UserDefined {
                name,
                representation,
            } => {
                // Explicitly dereference the Box to access the inner DepthGuarded value
                let inner = ScalarType::try_from((*representation).0)?;
                ScalarType::UserDefined {
                    name,
                    representation: Box::new(inner),
                }
            }
        };

        if ty.depth() > crate::types::MAX_TYPE_DEPTH {
            return Err(format!(
                "Type nesting too deep: {} (limit: {})",
                ty.depth(),
                crate::types::MAX_TYPE_DEPTH
            ));
        }
        Ok(ty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tuple_type::TupleType;
    use crate::types::relation_type::RelationType;

    #[test]
    fn should_treat_relations_with_different_headings_as_unequal() {
        let t1 = TupleType::new().with_attribute("a", ScalarType::Int);
        let rel1 = RelationType::new(t1);

        let t2 = TupleType::new().with_attribute("b", ScalarType::Int);
        let rel2 = RelationType::new(t2);

        let s1 = ScalarType::Relation(Box::new(rel1));
        let s2 = ScalarType::Relation(Box::new(rel2));

        assert!(s1 != s2);
        assert!(s1.cmp(&s2) != std::cmp::Ordering::Equal);
    }

    #[test]
    fn should_compare_relation_types_deterministically() {
        let t1 = TupleType::new()
            .with_attribute("a", ScalarType::Int)
            .with_attribute("b", ScalarType::String);
        let rel1 = RelationType::new(t1);

        let t2 = TupleType::new()
            .with_attribute("b", ScalarType::String)
            .with_attribute("a", ScalarType::Int);
        let rel2 = RelationType::new(t2);

        let s1 = ScalarType::Relation(Box::new(rel1));
        let s2 = ScalarType::Relation(Box::new(rel2));

        assert!(s1 == s2);
        assert!(s1.cmp(&s2) == std::cmp::Ordering::Equal);
    }
}
