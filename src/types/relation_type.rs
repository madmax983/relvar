use crate::types::TupleType;
use serde::{Deserialize, Serialize};

/// Relation type is defined by its heading, which is a tuple type.
/// Per Date's relational model, a relation is a set of tuples all of the same type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationType {
    heading: TupleType,
}

impl RelationType {
    /// Create a new relation type from a tuple type (heading)
    pub fn new(heading: TupleType) -> Self {
        Self { heading }
    }

    /// Get the heading (tuple type) of this relation
    pub fn heading(&self) -> &TupleType {
        &self.heading
    }

    /// Get the degree (number of attributes)
    pub fn degree(&self) -> usize {
        self.heading.degree()
    }

    /// Get the tuple type (heading)
    pub fn tuple_type(&self) -> &TupleType {
        &self.heading
    }

    /// Check if an attribute exists in the heading
    pub fn has_attribute(&self, name: &str) -> bool {
        self.heading.has_attribute(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ScalarType;

    #[test]
    fn test_relation_type_is_a_tuple_type() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading.clone());

        assert_eq!(rel_type.heading(), &heading);
        assert_eq!(rel_type.degree(), 2);
    }

    #[test]
    fn test_relation_type_equality() {
        let heading1 = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let heading2 = TupleType::new()
            .with_attribute("name", ScalarType::String)
            .with_attribute("emp_id", ScalarType::Int);

        let rel_type1 = RelationType::new(heading1);
        let rel_type2 = RelationType::new(heading2);

        assert_eq!(rel_type1, rel_type2);
    }

    #[test]
    fn test_empty_relation_type() {
        let empty_heading = TupleType::new();
        let rel_type = RelationType::new(empty_heading);

        assert_eq!(rel_type.degree(), 0);
    }
}
