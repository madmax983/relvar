import os

content = """//! Relational algebra delta operators for computing changes between relations.
//!
//! This module provides the [`Delta`] struct which represents the difference between
//! two states of a relation (an `old` state and a `new` state). It encapsulates both
//! the tuples that were inserted and the tuples that were deleted.
//!
//! # Why Delta?
//!
//! Computing a delta is essential for:
//! - **Materialized View Maintenance**: Incrementally updating views rather than recomputing them from scratch.
//! - **Triggers/Constraints**: Checking constraints only on the data that changed.
//! - **Replication/Logs**: Recording exactly what changed in a transaction.
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::Relation;
//! use relvar_core::algebra::delta::Delta;
//! use relvar_core::tuple;
//!
//! let heading = TupleType::new().with_attribute("id", ScalarType::Int);
//! let rel_type = RelationType::new(heading);
//!
//! let mut old = Relation::new(rel_type.clone());
//! old.insert(tuple! { id: 1i64 }).unwrap();
//! old.insert(tuple! { id: 2i64 }).unwrap();
//!
//! let mut new = Relation::new(rel_type);
//! new.insert(tuple! { id: 2i64 }).unwrap();
//! new.insert(tuple! { id: 3i64 }).unwrap();
//!
//! // Compute the delta (1 was deleted, 3 was inserted)
//! let delta = Delta::between(&old, &new).unwrap();
//! assert_eq!(delta.deleted.cardinality(), 1);
//! assert_eq!(delta.inserted.cardinality(), 1);
//!
//! // Apply the delta back to the old relation to get the new relation
//! let computed_new = delta.apply(&old).unwrap();
//! assert_eq!(computed_new, new);
//! ```

use crate::database::DatabaseError;
use crate::values::Relation;

/// Represents a change between two relations of the same type.
///
/// A `Delta` consists of two relations:
/// - `inserted`: The tuples that are in the new relation but not the old one.
/// - `deleted`: The tuples that are in the old relation but not the new one.
#[derive(Debug, Clone, PartialEq)]
pub struct Delta {
    /// Tuples added to the relation.
    pub inserted: Relation,
    /// Tuples removed from the relation.
    pub deleted: Relation,
}

impl Delta {
    /// Creates a new `Delta` from explicit inserted and deleted relations.
    ///
    /// The `inserted` and `deleted` relations must have exactly the same type
    /// (heading). If they don't, this returns a [`DatabaseError::AlgebraError`].
    ///
    /// # Parameters
    ///
    /// * `inserted` - Tuples to add
    /// * `deleted` - Tuples to remove
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::AlgebraError`] if the relations have different types.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::algebra::delta::Delta;
    ///
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    /// let inserted = Relation::new(rel_type.clone());
    /// let deleted = Relation::new(rel_type);
    ///
    /// let delta = Delta::new(inserted, deleted).unwrap();
    /// ```
    pub fn new(inserted: Relation, deleted: Relation) -> Result<Self, DatabaseError> {
        if inserted.relation_type() != deleted.relation_type() {
            return Err(DatabaseError::AlgebraError(
                "Type mismatch: inserted and deleted relations must have the same type".to_string(),
            ));
        }
        Ok(Self { inserted, deleted })
    }

    /// Computes the delta between two relations (`old` -> `new`).
    ///
    /// The resulting `Delta` will contain:
    /// - `inserted`: `new - old`
    /// - `deleted`: `old - new`
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::AlgebraError`] if the relations have different types.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::algebra::delta::Delta;
    /// use relvar_core::tuple;
    ///
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    ///
    /// let mut old = Relation::new(rel_type.clone());
    /// old.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// let mut new = Relation::new(rel_type);
    /// new.insert(tuple! { id: 2i64 }).unwrap();
    ///
    /// let delta = Delta::between(&old, &new).unwrap();
    /// assert!(delta.deleted.contains(&tuple! { id: 1i64 }));
    /// assert!(delta.inserted.contains(&tuple! { id: 2i64 }));
    /// ```
    pub fn between(old: &Relation, new: &Relation) -> Result<Self, DatabaseError> {
        if old.relation_type() != new.relation_type() {
            return Err(DatabaseError::AlgebraError(
                "Type mismatch: Cannot compute delta between relations of different types"
                    .to_string(),
            ));
        }

        // inserted = new - old
        let inserted = new
            .difference(old)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // deleted = old - new
        let deleted = old
            .difference(new)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(Self { inserted, deleted })
    }

    /// Applies this delta to a relation.
    ///
    /// The result is `(relation - deleted) U inserted`. This is the core equation
    /// for how a delta transforms state.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::AlgebraError`] if the relation type does not match the delta type.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::algebra::delta::Delta;
    /// use relvar_core::tuple;
    ///
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    ///
    /// let mut old = Relation::new(rel_type.clone());
    /// old.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// let mut new = Relation::new(rel_type.clone());
    /// new.insert(tuple! { id: 2i64 }).unwrap();
    ///
    /// let delta = Delta::between(&old, &new).unwrap();
    ///
    /// // Apply the delta to the old relation
    /// let result = delta.apply(&old).unwrap();
    /// assert_eq!(result, new);
    /// ```
    pub fn apply(&self, relation: &Relation) -> Result<Relation, DatabaseError> {
        if relation.relation_type() != self.inserted.relation_type() {
            return Err(DatabaseError::AlgebraError(
                "Type mismatch: Cannot apply delta to relation of different type".to_string(),
            ));
        }

        // relation - deleted
        let temp = relation
            .difference(&self.deleted)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // temp U inserted
        let result = temp
            .union(&self.inserted)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(result)
    }

    /// Inverts the delta.
    ///
    /// Returns a new `Delta` that undoes the effect of this one. This is useful for
    /// rolling back operations.
    /// - `inserted` becomes `deleted`
    /// - `deleted` becomes `inserted`
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::algebra::delta::Delta;
    /// use relvar_core::tuple;
    ///
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    ///
    /// let mut old = Relation::new(rel_type.clone());
    /// old.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// let mut new = Relation::new(rel_type.clone());
    /// new.insert(tuple! { id: 2i64 }).unwrap();
    ///
    /// let delta = Delta::between(&old, &new).unwrap();
    /// let inverse_delta = delta.invert();
    ///
    /// // Applying the inverse delta to the new relation restores the old one
    /// let restored_old = inverse_delta.apply(&new).unwrap();
    /// assert_eq!(restored_old, old);
    /// ```
    pub fn invert(&self) -> Self {
        Self {
            inserted: self.deleted.clone(),
            deleted: self.inserted.clone(),
        }
    }

    /// Composes this delta with another delta.
    ///
    /// If `self` transforms `R1 -> R2` and `other` transforms `R2 -> R3`,
    /// then `self.compose(other)` returns a `Delta` that transforms `R1 -> R3`.
    ///
    /// This is mathematically equivalent to:
    /// - `inserted_new = (self.inserted - other.deleted) U other.inserted`
    /// - `deleted_new = (self.deleted - other.inserted) U other.deleted`
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::AlgebraError`] if the deltas have different types.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar_core::algebra::delta::Delta;
    /// use relvar_core::tuple;
    ///
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    ///
    /// let mut r1 = Relation::new(rel_type.clone());
    /// r1.insert(tuple! { id: 1i64 }).unwrap();
    ///
    /// let mut r2 = Relation::new(rel_type.clone());
    /// r2.insert(tuple! { id: 2i64 }).unwrap();
    ///
    /// let mut r3 = Relation::new(rel_type.clone());
    /// r3.insert(tuple! { id: 3i64 }).unwrap();
    ///
    /// let delta_1_to_2 = Delta::between(&r1, &r2).unwrap();
    /// let delta_2_to_3 = Delta::between(&r2, &r3).unwrap();
    ///
    /// // Compose the deltas
    /// let delta_1_to_3 = delta_1_to_2.compose(&delta_2_to_3).unwrap();
    ///
    /// // Apply the composed delta
    /// let result = delta_1_to_3.apply(&r1).unwrap();
    /// assert_eq!(result, r3);
    /// ```
    pub fn compose(&self, other: &Self) -> Result<Self, DatabaseError> {
        if self.inserted.relation_type() != other.inserted.relation_type() {
            return Err(DatabaseError::AlgebraError(
                "Type mismatch: Cannot compose deltas of different types".to_string(),
            ));
        }

        // inserted_new = (self.inserted - other.deleted) U (other.inserted - self.deleted)
        // Note: The second term (I2 - D1) ensures that if a tuple was deleted by D1 (was in R1)
        // and then re-inserted by D2 (is in R3), it is not considered a "new" insertion relative to R1.
        let i_part1 = self
            .inserted
            .difference(&other.deleted)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let i_part2 = other
            .inserted
            .difference(&self.deleted)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let inserted_new = i_part1
            .union(&i_part2)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // deleted_new = (self.deleted - other.inserted) U (other.deleted - self.inserted)
        // Note: The second term (D2 - I1) ensures that if a tuple was inserted by D1 (transient)
        // and then deleted by D2, it is not considered "deleted" relative to R1.
        let d_part1 = self
            .deleted
            .difference(&other.inserted)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let d_part2 = other
            .deleted
            .difference(&self.inserted)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let deleted_new = d_part1
            .union(&d_part2)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(Self {
            inserted: inserted_new,
            deleted: deleted_new,
        })
    }
}
"""

with open('relvar-core/src/algebra/delta.rs', 'w') as f:
    f.write(content)
