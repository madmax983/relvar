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
/// # Examples
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
#[serde(try_from = "RelationTypeUnchecked")]
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int);
    ///
    /// let rel_type = RelationType::new(heading);
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::Int)
    ///     .with_attribute("name", ScalarType::String);
    ///
    /// let rel_type = RelationType::new(heading);
    /// assert_eq!(rel_type.degree(), 2);
    /// ```
    pub fn new(heading: TupleType) -> Self {
        if heading.depth() + 1 > crate::types::MAX_TYPE_DEPTH {
            panic!(
                "Type nesting too deep: {} (limit: {})",
                heading.depth() + 1,
                crate::types::MAX_TYPE_DEPTH
            );
        }
        Self { heading }
    }

    /// Calculates the nesting depth of this relation type.
    ///
    /// This is 1 + the depth of its heading.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let rel_type = RelationType::new(TupleType::new());
    /// assert_eq!(rel_type.depth(), 2); // Tuple (1) + Relation (1)
    /// ```
    pub fn depth(&self) -> usize {
        1 + self.heading.depth()
    }

    /// Retrieves the heading (tuple type) of this relation type.
    ///
    /// The heading defines what attributes tuples in this relation must have.
    ///
    /// # Examples
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    ///
    /// assert_eq!(rel_type.heading().degree(), 1);
    /// ```
    pub fn heading(&self) -> &TupleType {
        &self.heading
    }

    /// Calculates the degree (number of attributes) of this relation type.
    ///
    /// This is equivalent to `self.heading().degree()`.
    ///
    /// # Examples
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("x", ScalarType::Int)
    ///     .with_attribute("y", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    ///
    /// assert_eq!(rel_type.degree(), 2);
    /// ```
    pub fn degree(&self) -> usize {
        self.heading.degree()
    }

    /// Retrieves the tuple type (heading) of this relation type.
    ///
    /// This is an alias for [`heading()`](Self::heading).
    ///
    /// # Examples
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    ///
    /// assert_eq!(rel_type.tuple_type().degree(), 1);
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
    /// # Examples
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    ///
    /// assert!(rel_type.has_attribute("id"));
    /// assert!(!rel_type.has_attribute("name"));
    /// ```
    pub fn has_attribute(&self, name: &str) -> bool {
        self.heading.has_attribute(name)
    }
}

/// Wire shape of [`RelationType`], validated by `TryFrom` (same field name,
/// so the serialized format is unchanged).
#[derive(Deserialize)]
struct RelationTypeUnchecked {
    heading: TupleType,
}

impl TryFrom<RelationTypeUnchecked> for RelationType {
    type Error = crate::types::TypeDepthError;

    /// Enforces the same depth limit as [`RelationType::new`], returning an
    /// error where the constructor would panic.
    fn try_from(unchecked: RelationTypeUnchecked) -> Result<Self, Self::Error> {
        let relation_type = Self {
            heading: unchecked.heading,
        };
        crate::types::TypeDepthError::check(relation_type.depth())?;
        Ok(relation_type)
    }
}

impl PartialOrd for RelationType {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// A deterministic total order consistent with `Eq`: the order of the
/// headings (see [`TupleType`]'s `Ord`).
impl Ord for RelationType {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.heading.cmp(&other.heading)
    }
}

impl core::hash::Hash for RelationType {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
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

    /// `RelationType::new` panics on a heading whose relation type would be
    /// deeper than `MAX_TYPE_DEPTH`; deserialization must reject it with an
    /// error instead.
    #[test]
    fn deserialize_rejects_relation_type_deeper_than_max_depth() {
        let max = crate::types::MAX_TYPE_DEPTH;
        let mut ty = ScalarType::Int;
        for _ in 2..max {
            ty = ScalarType::user_defined("Nested", ty);
        }
        // Heading of depth `max` is constructible; wrapping it is not.
        let heading = TupleType::new().with_attribute("a", ty);
        assert_eq!(heading.depth(), max);
        assert!(
            RelationType::try_from(RelationTypeUnchecked {
                heading: heading.clone()
            })
            .is_err()
        );
        let too_deep = RelationType { heading };

        let json = serde_json::to_string(&too_deep).unwrap();
        assert!(serde_json::from_str::<RelationType>(&json).is_err());
        let bytes = postcard::to_allocvec(&too_deep).unwrap();
        assert!(postcard::from_bytes::<RelationType>(&bytes).is_err());
    }

    #[test]
    fn deserialize_accepts_relation_type_at_max_depth() {
        let max = crate::types::MAX_TYPE_DEPTH;
        let mut ty = ScalarType::Int;
        for _ in 3..max {
            ty = ScalarType::user_defined("Nested", ty);
        }
        let rel_type = RelationType::new(TupleType::new().with_attribute("a", ty));
        assert_eq!(rel_type.depth(), max);
        // Checked through `TryFrom` directly (see the tuple-type test).
        let unchecked = RelationTypeUnchecked {
            heading: rel_type.heading().clone(),
        };
        assert_eq!(RelationType::try_from(unchecked).unwrap(), rel_type);
    }
}
