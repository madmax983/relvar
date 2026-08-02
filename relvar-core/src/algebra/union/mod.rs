//! Union operator for combining relations.
//!
//! The union operator combines all tuples from two type-compatible relations
//! into a single relation. As required by set semantics, duplicate tuples
//! are automatically eliminated.
//!
//! # TTM Compliance
//!
//! - Relations must be type-compatible (identical headings)
//! - Duplicates are automatically eliminated (set semantics)
//! - Result is a valid relation
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::Relation;
//! use relvar_core::tuple;
//!
//! let heading = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//!
//! let rel_type = RelationType::new(heading);
//!
//! let mut rel1 = Relation::new(rel_type.clone());
//! rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
//!
//! let mut rel2 = Relation::new(rel_type);
//! rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
//!
//! let result = rel1.union(&rel2).unwrap();
//! assert_eq!(result.cardinality(), 2);  // Alice and Bob
//! ```

use crate::values::Relation;
use thiserror::Error;

/// The strict rules of relational matrimony.
///
/// In relational theory, two sets can only be united if they speak the exact same language.
/// This error is returned when an attempt is made to `union` two relations that are not
/// type-compatible.
///
/// # Recovery
/// - Ensure both relations have the exact same degree (number of attributes).
/// - Ensure both relations have the exact same attribute names.
/// - Ensure the scalar types of those attributes match perfectly.
/// - Use `project` or `rename` to align their headings before attempting the union.
///
/// # Examples
///
/// ```
/// use relvar_core::algebra::UnionError;
///
/// // The classic blunder: attempting to union apples and oranges.
/// // The recovery action is to align their headings first using `rename` or `project`.
/// let error = UnionError;
/// assert!(error.to_string().contains("Relations must have the same type"));
/// ```
#[derive(Debug, Error)]
#[error("Relations must have the same type (heading) for union")]
pub struct UnionError;

impl Relation {
    /// Computes the union of this relation with another.
    ///
    /// This is the set union operator from relational algebra. It produces a
    /// new relation containing all tuples that appear in either relation.
    /// Duplicate tuples are automatically eliminated per set semantics.
    ///
    /// # Arguments
    ///
    /// * `other` - The relation to union with. Must have the same heading
    ///   (type) as this relation.
    ///
    /// # Returns
    ///
    /// A new relation containing all tuples from both relations, with
    /// duplicates removed.
    ///
    /// # Errors
    ///
    /// Returns [`UnionError`] if the relations have different
    /// headings (different attribute names or types).
    ///
    /// # Behavior
    ///
    /// - Both relations must be type-compatible (identical headings)
    /// - Duplicate tuples across relations are eliminated
    /// - Union with self returns an equivalent relation
    /// - Union with empty relation returns the non-empty relation
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new()
    ///     .with_attribute("emp_id", ScalarType::Int)
    ///     .with_attribute("name", ScalarType::String);
    ///
    /// let rel_type = RelationType::new(heading);
    ///
    /// let mut rel1 = Relation::new(rel_type.clone());
    /// rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
    /// rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
    ///
    /// let mut rel2 = Relation::new(rel_type);
    /// rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();  // Duplicate
    /// rel2.insert(tuple! { emp_id: 3i64, name: "Charlie" }).unwrap();
    ///
    /// let result = rel1.union(&rel2).unwrap();
    /// assert_eq!(result.cardinality(), 3);  // Alice, Bob (once), Charlie
    /// ```
    pub fn union(&self, other: &Relation) -> Result<Self, UnionError> {
        // Check type compatibility
        if self.relation_type() != other.relation_type() {
            return Err(UnionError);
        }

        // Optimization: If one relation is empty, we just need to clone the other
        // and avoid instantiating iterators or chaining.
        if self.is_empty() {
            return Ok(other.clone());
        }
        if other.is_empty() {
            return Ok(self.clone());
        }

        let (larger, smaller) = if self.cardinality() >= other.cardinality() {
            (self, other)
        } else {
            (other, self)
        };

        // Optimization: By cloning the larger relation and extending it with the smaller,
        // we ensure that the underlying capacity is properly reserved from the start,
        // and insertions are optimally handled. `union_into` achieves this exact behavior.
        larger.clone().union_into(smaller)
    }

    /// Computes the union of this relation with another, consuming this relation.
    ///
    /// This is an optimized version of `union` that avoids O(N) tuple clones for the
    /// first relation by consuming it.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::tuple;
    ///
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    ///
    /// let mut rel1 = Relation::new(rel_type.clone());
    /// rel1.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// let mut rel2 = Relation::new(rel_type);
    /// rel2.insert(tuple! { id: 2i64 }).unwrap();
    ///
    /// let result = rel1.union_into(&rel2).unwrap();
    /// assert_eq!(result.cardinality(), 2);
    /// ```
    pub fn union_into(mut self, other: &Relation) -> Result<Self, UnionError> {
        // Check type compatibility
        if self.relation_type() != other.relation_type() {
            return Err(UnionError);
        }

        self.body.reserve(other.cardinality());
        self.body.extend(other.tuples().cloned());

        Ok(self)
    }
}

#[cfg(test)]
mod tests;
