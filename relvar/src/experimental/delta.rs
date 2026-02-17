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
//! use relvar::experimental::delta::Delta;
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

use relvar_core::error::DatabaseError;
use relvar_core::values::Relation;

/// Represents the difference between two relations.
///
/// A `Delta` consists of two sets of tuples:
/// - `inserted`: Tuples present in the new relation but not the old.
/// - `deleted`: Tuples present in the old relation but not the new.
///
/// Applying a `Delta` to a relation `R` results in `(R - deleted) U inserted`.
#[derive(Debug, Clone, PartialEq)]
pub struct Delta {
    /// Tuples to be inserted.
    pub inserted: Relation,
    /// Tuples to be deleted.
    pub deleted: Relation,
}

impl Delta {
    /// Creates a new `Delta` from explicit inserted and deleted relations.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::AlgebraError` if the two relations have different types.
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
    /// Returns a new `Delta` that undoes the effect of this one.
    /// - `inserted` becomes `deleted`
    /// - `deleted` becomes `inserted`
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

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    fn test_type() -> RelationType {
        RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int))
    }

    #[test]
    fn test_delta_between_empty() {
        let t = test_type();
        let r1 = Relation::new(t.clone());
        let r2 = Relation::new(t.clone());

        let delta = Delta::between(&r1, &r2).unwrap();
        assert!(delta.inserted.is_empty());
        assert!(delta.deleted.is_empty());
    }

    #[test]
    fn test_delta_simple() {
        let t = test_type();
        let mut r1 = Relation::new(t.clone());
        r1.insert(tuple! { x: 1i64 }).unwrap();

        let mut r2 = Relation::new(t.clone());
        r2.insert(tuple! { x: 2i64 }).unwrap();

        // r1 -> r2: delete 1, insert 2
        let delta = Delta::between(&r1, &r2).unwrap();
        assert_eq!(delta.deleted.cardinality(), 1);
        assert!(delta.deleted.contains(&tuple! { x: 1i64 }));
        assert_eq!(delta.inserted.cardinality(), 1);
        assert!(delta.inserted.contains(&tuple! { x: 2i64 }));

        // Apply
        let r3 = delta.apply(&r1).unwrap();
        assert_eq!(r3, r2);
    }

    #[test]
    fn test_invert() {
        let t = test_type();
        let mut r1 = Relation::new(t.clone());
        r1.insert(tuple! { x: 1i64 }).unwrap();

        let mut r2 = Relation::new(t.clone());
        r2.insert(tuple! { x: 2i64 }).unwrap();

        let delta = Delta::between(&r1, &r2).unwrap();
        let inv = delta.invert();

        // inv should be r2 -> r1: delete 2, insert 1
        assert!(inv.deleted.contains(&tuple! { x: 2i64 }));
        assert!(inv.inserted.contains(&tuple! { x: 1i64 }));

        let r_back = inv.apply(&r2).unwrap();
        assert_eq!(r_back, r1);
    }

    #[test]
    fn test_compose() {
        let t = test_type();

        // R1: {1}
        let mut r1 = Relation::new(t.clone());
        r1.insert(tuple! { x: 1i64 }).unwrap();

        // R2: {2}
        let mut r2 = Relation::new(t.clone());
        r2.insert(tuple! { x: 2i64 }).unwrap();

        // R3: {3}
        let mut r3 = Relation::new(t.clone());
        r3.insert(tuple! { x: 3i64 }).unwrap();

        // D1: R1 -> R2 (del 1, ins 2)
        let d1 = Delta::between(&r1, &r2).unwrap();

        // D2: R2 -> R3 (del 2, ins 3)
        let d2 = Delta::between(&r2, &r3).unwrap();

        // D3: R1 -> R3 (should be del 1, ins 3)
        let d3 = d1.compose(&d2).unwrap();

        assert!(d3.deleted.contains(&tuple! { x: 1i64 }));
        assert!(!d3.deleted.contains(&tuple! { x: 2i64 })); // 2 was ins then del, so net zero

        assert!(d3.inserted.contains(&tuple! { x: 3i64 }));
        assert!(!d3.inserted.contains(&tuple! { x: 2i64 })); // 2 was ins then del, so net zero

        let r_final = d3.apply(&r1).unwrap();
        assert_eq!(r_final, r3);
    }

    #[test]
    fn test_compose_overlapping() {
        // R1: {}
        // R2: {1} (D1: ins 1)
        // R3: {1, 2} (D2: ins 2)
        // D3 = D1 + D2 -> ins {1, 2}

        let t = test_type();
        let r1 = Relation::new(t.clone());

        let mut r2 = Relation::new(t.clone());
        r2.insert(tuple! { x: 1i64 }).unwrap();

        let mut r3 = Relation::new(t.clone());
        r3.insert(tuple! { x: 1i64 }).unwrap();
        r3.insert(tuple! { x: 2i64 }).unwrap();

        let d1 = Delta::between(&r1, &r2).unwrap();
        let d2 = Delta::between(&r2, &r3).unwrap();

        let d3 = d1.compose(&d2).unwrap();

        assert_eq!(d3.inserted.cardinality(), 2);
        assert!(d3.inserted.contains(&tuple! { x: 1i64 }));
        assert!(d3.inserted.contains(&tuple! { x: 2i64 }));

        let r_final = d3.apply(&r1).unwrap();
        assert_eq!(r_final, r3);
    }
}
