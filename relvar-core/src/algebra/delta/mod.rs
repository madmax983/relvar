//! Relational Delta: A module for computing and applying differences between relations.
//!
//! This module provides the `Delta` struct, which represents the difference between two relations
//! as a set of inserted and deleted tuples. It allows for:
//! - Computing the difference between two relations (`Delta::between`).
//! - Applying a delta to a relation (`Delta::apply`).
//! - Inverting a delta (`Delta::invert`).
//! - Composing two deltas (`Delta::compose`).
//!
//! # Example
//!
//! ```
//! use relvar_core::values::Relation;
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use relvar_core::tuple;
//! use relvar_core::algebra::Delta;
//!
//! let heading = TupleType::new().with_attribute("x", ScalarType::Int);
//! let rel_type = RelationType::new(heading);
//!
//! let mut r1 = Relation::new(rel_type.clone());
//! r1.insert(tuple! { x: 1i64 }).unwrap();
//!
//! let mut r2 = Relation::new(rel_type.clone());
//! r2.insert(tuple! { x: 2i64 }).unwrap();
//!
//! // Compute delta: r1 -> r2
//! // This means: delete (1), insert (2)
//! let delta = Delta::between(&r1, &r2).unwrap();
//!
//! // Apply delta to r1
//! let r3 = delta.apply(&r1).unwrap();
//! assert_eq!(r3, r2);
//! ```

use crate::error::DatabaseError;
use crate::values::Relation;

/// Represents the difference between two relations.
///
/// A `Delta` consists of two sets of tuples:
/// - `inserted`: Tuples present in the new relation but not the old.
/// - `deleted`: Tuples present in the old relation but not the new.
///
/// Applying a `Delta` to a relation `R` results in `(R - deleted) U inserted`.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct Delta {
    /// Tuples to be inserted.
    pub inserted: Relation,
    /// Tuples to be deleted.
    pub deleted: Relation,
}

#[allow(dead_code)]
impl Delta {
    /// Creates a new `Delta` from explicit inserted and deleted relations.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::AlgebraError` if the two relations have different types.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::algebra::Delta;
    ///
    /// let heading = TupleType::new().with_attribute("x", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// let r1 = Relation::new(rel_type.clone());
    /// let r2 = Relation::new(rel_type);
    ///
    /// let delta = Delta::new(r1, r2).unwrap();
    /// ```
    pub fn new(inserted: Relation, deleted: Relation) -> Result<Self, DatabaseError> {
        if inserted.relation_type() != deleted.relation_type() {
            return Err(DatabaseError::AlgebraError(
                "Type mismatch: Inserted and deleted relations must have the same type".to_string(),
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
    /// Returns `DatabaseError::AlgebraError` if the relations have different types.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::algebra::Delta;
    ///
    /// let heading = TupleType::new().with_attribute("x", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// let r1 = Relation::new(rel_type.clone());
    /// let r2 = Relation::new(rel_type);
    ///
    /// let delta = Delta::between(&r1, &r2).unwrap();
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
    /// The result is `(relation - deleted) U inserted`.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::AlgebraError` if the relation type does not match the delta type.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::algebra::Delta;
    ///
    /// let heading = TupleType::new().with_attribute("x", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// let r1 = Relation::new(rel_type.clone());
    /// let r2 = Relation::new(rel_type);
    ///
    /// let delta = Delta::between(&r1, &r2).unwrap();
    /// let applied = delta.apply(&r1).unwrap();
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
            .union_into(&self.inserted)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(result)
    }

    /// Inverts the delta.
    ///
    /// Yields a new `Delta` that undoes the effect of this one.
    /// - `inserted` becomes `deleted`
    /// - `deleted` becomes `inserted`
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::algebra::Delta;
    ///
    /// let heading = TupleType::new().with_attribute("x", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// let r1 = Relation::new(rel_type.clone());
    /// let r2 = Relation::new(rel_type);
    ///
    /// let delta = Delta::between(&r1, &r2).unwrap();
    /// let inverted = delta.invert();
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
    /// Formula:
    /// - `inserted_new = (self.inserted - other.deleted) U other.inserted`
    /// - `deleted_new = (self.deleted - other.inserted) U other.deleted`
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::AlgebraError` if the deltas have different types.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::algebra::Delta;
    ///
    /// let heading = TupleType::new().with_attribute("x", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// let r1 = Relation::new(rel_type.clone());
    /// let r2 = Relation::new(rel_type);
    ///
    /// let d1 = Delta::between(&r1, &r2).unwrap();
    /// let d2 = Delta::between(&r1, &r2).unwrap();
    /// let composed = d1.compose(&d2).unwrap();
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
            .union_into(&i_part2)
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
            .union_into(&d_part2)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(Self {
            inserted: inserted_new,
            deleted: deleted_new,
        })
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod additional_delta_tests {
    use super::*;
    use crate::types::{RelationType, ScalarType, TupleType};

    fn test_type_1() -> RelationType {
        RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int))
    }

    fn test_type_2() -> RelationType {
        RelationType::new(TupleType::new().with_attribute("y", ScalarType::Int))
    }

    #[test]
    fn test_delta_new_type_mismatch_internal() {
        let r1 = Relation::new(test_type_1());
        let r2 = Relation::new(test_type_2());

        let result = Delta::new(r1, r2);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }

    #[test]
    fn test_delta_between_type_mismatch_internal() {
        let r1 = Relation::new(test_type_1());
        let r2 = Relation::new(test_type_2());

        let result = Delta::between(&r1, &r2);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }

    #[test]
    fn test_delta_apply_type_mismatch_internal() {
        let r1 = Relation::new(test_type_1());
        let r2 = Relation::new(test_type_1());
        let r3 = Relation::new(test_type_2());

        let delta = Delta::between(&r1, &r2).unwrap();
        let result = delta.apply(&r3);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }

    #[test]
    fn test_delta_compose_type_mismatch_internal() {
        let r1 = Relation::new(test_type_1());
        let r2 = Relation::new(test_type_1());
        let r3 = Relation::new(test_type_2());
        let r4 = Relation::new(test_type_2());

        let d1 = Delta::between(&r1, &r2).unwrap();
        let d2 = Delta::between(&r3, &r4).unwrap();

        let result = d1.compose(&d2);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }
}
