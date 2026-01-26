use crate::types::ScalarType;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Tuple type is defined by a set of (attribute_name, type) pairs.
/// Per Date's relational model, attributes have no ordering.
/// We use BTreeMap for deterministic iteration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TupleType {
    /// Attributes of this tuple type, mapping attribute names to their types
    attributes: BTreeMap<String, ScalarType>,
}

impl TupleType {
    /// Create a new empty tuple type
    pub fn new() -> Self {
        Self {
            attributes: BTreeMap::new(),
        }
    }

    /// Add an attribute to the tuple type
    pub fn with_attribute(mut self, name: impl Into<String>, ty: ScalarType) -> Self {
        self.attributes.insert(name.into(), ty);
        self
    }

    /// Get the type of an attribute by name
    pub fn get_attribute_type(&self, name: &str) -> Option<&ScalarType> {
        self.attributes.get(name)
    }

    /// Check if an attribute exists
    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes.contains_key(name)
    }

    /// Get all attribute names
    pub fn attribute_names(&self) -> impl Iterator<Item = &String> {
        self.attributes.keys()
    }

    /// Get the number of attributes (degree)
    pub fn degree(&self) -> usize {
        self.attributes.len()
    }

    /// Get all attributes
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
