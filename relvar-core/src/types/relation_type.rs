//! Relation types for the relational model.
//!
//! A relation type defines the structure of a relation. It consists of a
//! heading (tuple type) that specifies the attributes all tuples in the
//! relation must have.
//!
//! # TTM Compliance
//!
//! - A relation is a set of tuples all of the same type
//! - The relation type is determined solely by its heading
//! - Type compatibility for set operations requires identical headings
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//!
//! let employee_heading = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//!
//! let employee_type = RelationType::new(employee_heading);
//!
//! assert_eq!(employee_type.degree(), 2);
//! assert!(employee_type.has_attribute("emp_id"));
//! ```

use crate::types::TupleType;
use serde::{Deserialize, Serialize};

/// Defines the type of a relation.
///
/// A `RelationType` specifies what attributes a relation contains and their
/// types. All tuples in a relation must conform to the relation's type.
///
/// The relation type is defined entirely by its "heading" - the tuple type
/// that describes the structure of its tuples.
///
/// # Type Compatibility
///
/// Two relations are type-compatible if and only if they have the same
/// relation type (identical headings). This is required for set operations
/// like union, intersection, and difference.
///
/// # Example
///
/// ```
/// use relvar_core::types::{RelationType, TupleType, ScalarType};
///
/// // Define a relation type for employees
/// let heading = TupleType::new()
///     .with_attribute("emp_id", ScalarType::Int)
///     .with_attribute("name", ScalarType::String)
///     .with_attribute("dept_id", ScalarType::Int);
///
/// let employee_type = RelationType::new(heading);
///
/// // Query the type
/// assert_eq!(employee_type.degree(), 3);
/// assert!(employee_type.has_attribute("emp_id"));
/// assert!(!employee_type.has_attribute("salary"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationType {
    /// The heading (tuple type) that defines this relation type.
    heading: TupleType,
}

impl RelationType {
    /// Creates a new relation type from a tuple type (heading).
    ///
    /// The heading specifies the attributes that all tuples in relations
    /// of this type must have.
    ///
    /// # Arguments
    ///
    /// * `heading` - The tuple type defining the relation's structure
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let rel_type = RelationType::new(heading);
    /// ```
    pub fn new(heading: TupleType) -> Self {
        Self { heading }
    }

    /// Returns the heading (tuple type) of this relation type.
    ///
    /// The heading defines what attributes tuples in this relation must have.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let rel_type = RelationType::new(heading.clone());
    /// assert_eq!(rel_type.heading(), &heading);
    /// ```
    pub fn heading(&self) -> &TupleType {
        &self.heading
    }

    /// Returns the degree (number of attributes) of this relation type.
    ///
    /// This is equivalent to `self.heading().degree()`.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("a", ScalarType::Int)
    ///     .with_attribute("b", ScalarType::Int);
    ///
    /// let rel_type = RelationType::new(heading);
    /// assert_eq!(rel_type.degree(), 2);
    /// ```
    pub fn degree(&self) -> usize {
        self.heading.degree()
    }

    /// Returns the tuple type (heading) of this relation type.
    ///
    /// This is an alias for [`heading()`](Self::heading).
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let rel_type = RelationType::new(heading.clone());
    /// assert_eq!(rel_type.tuple_type(), &heading);
    /// ```
    pub fn tuple_type(&self) -> &TupleType {
        &self.heading
    }

    /// Checks whether an attribute with the given name exists in the heading.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute name to check
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let rel_type = RelationType::new(heading);
    /// assert!(rel_type.has_attribute("id"));
    /// assert!(!rel_type.has_attribute("name"));
    /// ```
    pub fn has_attribute(&self, name: &str) -> bool {
        self.heading.has_attribute(name)
    }
}

impl std::hash::Hash for RelationType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Just hash the heading
        self.heading.hash(state);
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
