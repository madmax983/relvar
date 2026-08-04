//! Difference Operator implementation.
//!
//! This module provides the set difference operator for relational algebra.

use crate::values::Relation;
use thiserror::Error;

/// Errors that can occur during difference operations.
#[derive(Debug, Error)]
#[error("Relations must have the same type (heading) for difference")]
pub struct DifferenceError;

impl Relation {
    /// Computes the set difference of this relation with another.
    ///
    /// This is the set difference operator from relational algebra (A - B or
    /// A MINUS B in SQL). It produces a new relation containing only the tuples
    /// that appear in this relation but not in the other relation.
    ///
    /// # Arguments
    ///
    /// * `other` - The relation to subtract. Must have the same heading
    ///   (type) as this relation.
    ///
    /// # Returns
    ///
    /// A new relation containing tuples from `self` that are not in `other`.
    ///
    /// # Errors
    ///
    /// Returns [`DifferenceError`] if the relations have different
    /// headings (different attribute names or types).
    ///
    /// # Behavior
    ///
    /// - Both relations must be type-compatible (identical headings)
    /// - Only tuples present in `self` but not in `other` are included
    /// - Difference with empty relation returns a copy of self
    /// - Difference with self returns empty relation
    /// - If no common tuples exist, returns a copy of self
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
    /// let rel_type = RelationType::new(heading);
    ///
    /// let mut all = Relation::new(rel_type.clone());
    /// all.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
    /// all.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
    /// all.insert(tuple! { emp_id: 3i64, name: "Charlie" }).unwrap();
    ///
    /// let mut subset = Relation::new(rel_type);
    /// subset.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
    ///
    /// let result = all.difference(&subset).unwrap();
    /// assert_eq!(result.cardinality(), 2);  // Alice and Charlie
    /// ```
    pub fn difference(&self, other: &Relation) -> Result<Self, DifferenceError> {
        // Check type compatibility
        if self.relation_type() != other.relation_type() {
            return Err(DifferenceError);
        }

        // Optimization: If the other relation is empty, no tuples are removed.
        // By checking this early, we avoid O(N) hash lookups.
        if other.is_empty() {
            return Ok(self.clone());
        }

        // Optimization: If self is empty, the result is empty.
        // We avoid setting up iterators or cloning.
        if self.is_empty() {
            return Ok(self.clone());
        }

        // Filter tuples and create relation without redundant checks
        // Safety: source tuples are from a valid relation of the same type
        // Optimization: Pass the iterator directly to `from_tuples_unchecked` instead of
        // collecting into an intermediate `Vec`. This avoids allocating a temporary buffer
        // for the resulting tuples.
        Ok(Relation::from_tuples_unchecked(
            self.relation_type().clone(),
            self.tuples()
                .filter(|tuple| !other.contains(tuple))
                .cloned(),
        ))
    }

    /// Alias for [`difference`](Self::difference) with SQL-style naming.
    ///
    /// This method is identical to `difference()` but uses the SQL-style
    /// name "minus" which some users may find more intuitive.
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
    /// let rel_type = RelationType::new(heading);
    ///
    /// let mut rel1 = Relation::new(rel_type.clone());
    /// rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
    ///
    /// let mut rel2 = Relation::new(rel_type);
    /// rel2.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
    ///
    /// let result = rel1.minus(&rel2).unwrap();
    /// assert!(result.is_empty());
    /// ```
    pub fn minus(&self, other: &Relation) -> Result<Self, DifferenceError> {
        self.difference(other)
    }

    /// Computes the set difference of this relation with another, consuming this relation.
    ///
    /// This is an optimized version of `difference` that avoids O(N) tuple clones for the
    /// first relation by consuming it and mutates the inner body in place.
    ///
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
    /// let mut all = Relation::new(rel_type.clone());
    /// all.insert(tuple! { id: 1i64 }).unwrap();
    /// all.insert(tuple! { id: 2i64 }).unwrap();
    ///
    /// let mut subset = Relation::new(rel_type);
    /// subset.insert(tuple! { id: 2i64 }).unwrap();
    ///
    /// let result = all.difference_into(&subset).unwrap();
    /// assert_eq!(result.cardinality(), 1);
    /// ```
    pub fn difference_into(mut self, other: &Relation) -> Result<Self, DifferenceError> {
        // Check type compatibility
        if self.relation_type() != other.relation_type() {
            return Err(DifferenceError);
        }

        self.body.retain(|tuple| !other.contains(tuple));
        Ok(self)
    }

    /// Alias for [`difference_into`](Self::difference_into) with SQL-style naming.
    ///
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
    /// rel2.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// let result = rel1.minus_into(&rel2).unwrap();
    /// assert!(result.is_empty());
    /// ```
    pub fn minus_into(self, other: &Relation) -> Result<Self, DifferenceError> {
        self.difference_into(other)
    }
}

#[cfg(test)]
mod tests;
