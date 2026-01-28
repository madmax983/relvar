//! Tuple types (headings) for the relational model.
//!
//! A tuple type, also called a "heading" in relational terminology, defines
//! the structure of tuples in a relation. It is a set of attribute definitions,
//! where each attribute has a name and a scalar type.
//!
//! # TTM Compliance
//!
//! - **Proscription 4**: Attributes have no ordering (identified by name only)
//! - Attribute names must be unique within a tuple type
//! - Type equality is structural (same attributes = same type)
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, ScalarType};
//!
//! let employee_type = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String)
//!     .with_attribute("salary", ScalarType::Float);
//!
//! assert_eq!(employee_type.degree(), 3);
//! assert!(employee_type.has_attribute("emp_id"));
//! ```

use crate::types::ScalarType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Defines the structure (heading) of tuples.
///
/// A `TupleType` represents the set of attributes that tuples of this type
/// must have. Each attribute has a unique name and a scalar type. This is
/// called a "heading" in relational terminology.
///
/// # Attribute Ordering
///
/// Per TTM Proscription 4, attributes have no inherent ordering. They are
/// identified solely by name. The implementation uses `HashMap` to enforce
/// that there is no guaranteed ordering of attributes.
///
/// # Type Equality
///
/// Two tuple types are equal if and only if they have the same set of
/// attributes (same names with same types). The order in which attributes
/// were added does not affect equality.
///
/// # Example
///
/// ```
/// use relvar_core::types::{TupleType, ScalarType};
///
/// // Create a tuple type with the builder pattern
/// let person_type = TupleType::new()
///     .with_attribute("id", ScalarType::Int)
///     .with_attribute("name", ScalarType::String)
///     .with_attribute("active", ScalarType::Bool);
///
/// // Query the type
/// assert_eq!(person_type.degree(), 3);
/// assert_eq!(person_type.get_attribute_type("id"), Some(&ScalarType::Int));
/// assert!(!person_type.has_attribute("nonexistent"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TupleType {
    /// The attributes of this tuple type, mapping names to scalar types.
    attributes: HashMap<String, ScalarType>,
}

impl TupleType {
    /// Creates a new empty tuple type.
    ///
    /// An empty tuple type (degree 0) is valid and represents the type of
    /// tuples with no attributes. Such tuples are called "0-tuples" or
    /// "nullary tuples".
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::TupleType;
    ///
    /// let empty_type = TupleType::new();
    /// assert_eq!(empty_type.degree(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            attributes: HashMap::new(),
        }
    }

    /// Adds an attribute to this tuple type.
    ///
    /// This is a builder-style method that returns `self` for chaining.
    /// If an attribute with the same name already exists, it will be
    /// overwritten with the new type.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute name (must be unique within the tuple type)
    /// * `ty` - The scalar type of the attribute
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, ScalarType};
    ///
    /// let person_type = TupleType::new()
    ///     .with_attribute("name", ScalarType::String)
    ///     .with_attribute("age", ScalarType::Int);
    ///
    /// assert_eq!(person_type.degree(), 2);
    /// ```
    pub fn with_attribute(mut self, name: impl Into<String>, ty: ScalarType) -> Self {
        self.attributes.insert(name.into(), ty);
        self
    }

    /// Gets the type of an attribute by name.
    ///
    /// Returns `None` if no attribute with the given name exists.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute name to look up
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, ScalarType};
    ///
    /// let tuple_type = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// assert_eq!(tuple_type.get_attribute_type("id"), Some(&ScalarType::Int));
    /// assert_eq!(tuple_type.get_attribute_type("nonexistent"), None);
    /// ```
    pub fn get_attribute_type(&self, name: &str) -> Option<&ScalarType> {
        self.attributes.get(name)
    }

    /// Checks whether an attribute with the given name exists.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute name to check
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, ScalarType};
    ///
    /// let tuple_type = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// assert!(tuple_type.has_attribute("id"));
    /// assert!(!tuple_type.has_attribute("other"));
    /// ```
    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes.contains_key(name)
    }

    /// Returns an iterator over all attribute names.
    ///
    /// Per TTM Proscription 4, the order of iteration is arbitrary and
    /// should not be relied upon. Attributes are identified by name only.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, ScalarType};
    ///
    /// let tuple_type = TupleType::new()
    ///     .with_attribute("b", ScalarType::Int)
    ///     .with_attribute("a", ScalarType::Int);
    ///
    /// let names: Vec<_> = tuple_type.attribute_names().collect();
    /// assert_eq!(names.len(), 2);
    /// ```
    pub fn attribute_names(&self) -> impl Iterator<Item = &String> {
        self.attributes.keys()
    }

    /// Returns the degree (number of attributes) of this tuple type.
    ///
    /// The degree is the count of distinct attributes in the heading.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, ScalarType};
    ///
    /// let tuple_type = TupleType::new()
    ///     .with_attribute("a", ScalarType::Int)
    ///     .with_attribute("b", ScalarType::Int)
    ///     .with_attribute("c", ScalarType::Int);
    ///
    /// assert_eq!(tuple_type.degree(), 3);
    /// ```
    pub fn degree(&self) -> usize {
        self.attributes.len()
    }

    /// Returns a reference to the underlying attribute map.
    ///
    /// This provides access to all attribute definitions as a map from
    /// names to types.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, ScalarType};
    ///
    /// let tuple_type = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// for (name, scalar_type) in tuple_type.attributes() {
    ///     println!("{}: {:?}", name, scalar_type);
    /// }
    /// ```
    pub fn attributes(&self) -> &HashMap<String, ScalarType> {
        &self.attributes
    }
}

impl Default for TupleType {
    fn default() -> Self {
        Self::new()
    }
}

impl std::hash::Hash for TupleType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash in a deterministic way by sorting the entries
        let mut entries: Vec<_> = self.attributes.iter().collect();
        entries.sort_by_key(|(name, _)| *name);

        // Hash the count first
        self.attributes.len().hash(state);

        // Then hash each entry
        for (name, ty) in entries {
            name.hash(state);
            ty.hash(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tuple_type_defined_by_attributes() {
        let tuple_type = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("salary", ScalarType::Float);

        assert_eq!(tuple_type.degree(), 3);
        assert_eq!(
            tuple_type.get_attribute_type("emp_id"),
            Some(&ScalarType::Int)
        );
        assert_eq!(
            tuple_type.get_attribute_type("name"),
            Some(&ScalarType::String)
        );
        assert_eq!(
            tuple_type.get_attribute_type("salary"),
            Some(&ScalarType::Float)
        );
    }

    #[test]
    fn test_attribute_names_must_be_unique() {
        // The BTreeMap will simply overwrite duplicates
        let tuple_type = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("id", ScalarType::String); // Overwrites previous

        assert_eq!(tuple_type.degree(), 1);
        assert_eq!(
            tuple_type.get_attribute_type("id"),
            Some(&ScalarType::String)
        );
    }

    #[test]
    fn test_tuple_type_equality() {
        let type1 = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let type2 = TupleType::new()
            .with_attribute("name", ScalarType::String)
            .with_attribute("emp_id", ScalarType::Int); // Different order

        let type3 = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::Float); // Different type

        assert_eq!(type1, type2); // Order doesn't matter
        assert_ne!(type1, type3); // Different types
    }

    #[test]
    fn test_has_attribute() {
        let tuple_type = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        assert!(tuple_type.has_attribute("emp_id"));
        assert!(tuple_type.has_attribute("name"));
        assert!(!tuple_type.has_attribute("salary"));
    }

    /// TTM Proscription 4: Attributes have no ordering.
    ///
    /// This test verifies that we don't guarantee any particular ordering
    /// of attributes. The iteration order should be considered arbitrary
    /// and must not be relied upon for semantic purposes.
    #[test]
    fn test_ttm_proscription_4_no_attribute_ordering() {
        let tuple_type = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let names: Vec<_> = tuple_type.attribute_names().cloned().collect();

        // TTM Proscription 4: Attributes must have no ordering.
        // We verify all expected attributes are present, but we do NOT
        // check for any particular order. HashMap ensures no guaranteed ordering.
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"emp_id".to_string()));
        assert!(names.contains(&"name".to_string()));
        assert!(names.contains(&"dept_id".to_string()));
    }

    #[test]
    fn test_attribute_names_iterator() {
        let tuple_type = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let names: Vec<_> = tuple_type.attribute_names().cloned().collect();

        // TTM Proscription 4: Attributes have no ordering.
        // Verify all attributes are present, but don't check order.
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"emp_id".to_string()));
        assert!(names.contains(&"name".to_string()));
        assert!(names.contains(&"dept_id".to_string()));
    }

    #[test]
    fn test_empty_tuple_type() {
        let empty = TupleType::new();
        assert_eq!(empty.degree(), 0);
        assert!(!empty.has_attribute("anything"));
    }
}
