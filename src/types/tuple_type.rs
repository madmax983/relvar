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
//! use relvar::types::{TupleType, ScalarType};
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
use std::collections::BTreeMap;

/// Defines the structure (heading) of tuples.
///
/// A `TupleType` represents the set of attributes that tuples of this type
/// must have. Each attribute has a unique name and a scalar type. This is
/// called a "heading" in relational terminology.
///
/// # Attribute Ordering
///
/// Per TTM Proscription 4, attributes have no inherent ordering. They are
/// identified solely by name. The implementation uses `BTreeMap` internally
/// for deterministic iteration, but this ordering should not be relied upon
/// for semantic purposes.
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
/// use relvar::types::{TupleType, ScalarType};
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TupleType {
    /// The attributes of this tuple type, mapping names to scalar types.
    attributes: BTreeMap<String, ScalarType>,
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
    /// use relvar::types::TupleType;
    ///
    /// let empty_type = TupleType::new();
    /// assert_eq!(empty_type.degree(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            attributes: BTreeMap::new(),
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
    /// use relvar::types::{TupleType, ScalarType};
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
    /// use relvar::types::{TupleType, ScalarType};
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
    /// use relvar::types::{TupleType, ScalarType};
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
    /// The order of iteration is deterministic (sorted by name) but should
    /// not be relied upon for semantic purposes per TTM Proscription 4.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar::types::{TupleType, ScalarType};
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
    /// use relvar::types::{TupleType, ScalarType};
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
    /// use relvar::types::{TupleType, ScalarType};
    ///
    /// let tuple_type = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// for (name, scalar_type) in tuple_type.attributes() {
    ///     println!("{}: {:?}", name, scalar_type);
    /// }
    /// ```
    pub fn attributes(&self) -> &BTreeMap<String, ScalarType> {
        &self.attributes
    }
}

impl Default for TupleType {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn test_attribute_names_iterator() {
        let tuple_type = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let names: Vec<_> = tuple_type.attribute_names().cloned().collect();

        // BTreeMap maintains sorted order
        assert_eq!(names, vec!["dept_id", "emp_id", "name"]);
    }

    #[test]
    fn test_empty_tuple_type() {
        let empty = TupleType::new();
        assert_eq!(empty.degree(), 0);
        assert!(!empty.has_attribute("anything"));
    }
}
