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
use alloc::string::ToString;

/// Represents the difference between two relations.
///
/// A `Delta` consists of two disjoint sets of tuples of one relation type:
/// - `inserted`: Tuples present in the new relation but not the old.
/// - `deleted`: Tuples present in the old relation but not the new.
///
/// Applying a `Delta` to a relation `R` results in `(R - deleted) U inserted`.
///
/// # Invariants
///
/// `inserted` and `deleted` have the same relation type and no tuple in
/// common (a tuple cannot be both inserted and deleted). The fields are
/// private so that every `Delta` is built through [`Delta::new`] or
/// [`Delta::between`], which enforce this.
///
/// A delta is *effective* for a base relation `R` when every inserted tuple
/// is absent from `R` and every deleted tuple is present in `R` (see
/// [`Delta::is_effective_for`]). [`Delta::between`] always produces a delta
/// that is effective for its `old` relation.
#[derive(Debug, Clone, PartialEq)]
pub struct Delta {
    /// Tuples to be inserted.
    inserted: Relation,
    /// Tuples to be deleted.
    deleted: Relation,
}

impl Delta {
    /// The tuples this delta inserts.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::algebra::Delta;
    ///
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));
    /// let delta = Delta::between(&Relation::new(rel_type.clone()), &Relation::new(rel_type)).unwrap();
    /// assert!(delta.inserted().is_empty());
    /// ```
    pub fn inserted(&self) -> &Relation {
        &self.inserted
    }

    /// The tuples this delta deletes.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::algebra::Delta;
    ///
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));
    /// let delta = Delta::between(&Relation::new(rel_type.clone()), &Relation::new(rel_type)).unwrap();
    /// assert!(delta.deleted().is_empty());
    /// ```
    pub fn deleted(&self) -> &Relation {
        &self.deleted
    }

    /// Consumes the delta, returning `(inserted, deleted)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::algebra::Delta;
    ///
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));
    /// let delta = Delta::between(&Relation::new(rel_type.clone()), &Relation::new(rel_type)).unwrap();
    /// let (inserted, deleted) = delta.into_parts();
    /// assert!(inserted.is_empty() && deleted.is_empty());
    /// ```
    pub fn into_parts(self) -> (Relation, Relation) {
        (self.inserted, self.deleted)
    }

    /// Returns `true` if this delta is effective for `base`: every inserted
    /// tuple is absent from `base` and every deleted tuple is present in it.
    ///
    /// [`Delta::between`] produces deltas that are effective for their `old`
    /// relation. [`compose`](Self::compose) is exact whenever `self` is
    /// effective for the base the composed delta is applied to.
    ///
    /// Returns `false` if `base` has a different relation type.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::values::Relation;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::algebra::Delta;
    /// use relvar_core::tuple;
    ///
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));
    /// let old = Relation::new(rel_type.clone());
    /// let mut new = Relation::new(rel_type);
    /// new.insert(tuple! { x: 1i64 }).unwrap();
    ///
    /// let delta = Delta::between(&old, &new).unwrap();
    /// assert!(delta.is_effective_for(&old));
    /// assert!(!delta.is_effective_for(&new)); // 1 is already in `new`
    /// ```
    pub fn is_effective_for(&self, base: &Relation) -> bool {
        base.relation_type() == self.inserted.relation_type()
            && self.inserted.tuples().all(|t| !base.contains(t))
            && self.deleted.tuples().all(|t| base.contains(t))
    }

    /// Creates a new `Delta` from explicit inserted and deleted relations.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::AlgebraError` if the two relations have
    /// different types, or if some tuple is both inserted and deleted.
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
        if inserted.tuples().any(|t| deleted.contains(t)) {
            return Err(DatabaseError::AlgebraError(
                "A tuple cannot be both inserted and deleted by one delta".to_string(),
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
    /// - `inserted_new = (self.inserted - other.deleted) U (other.inserted - self.deleted)`
    /// - `deleted_new = (self.deleted - other.inserted) U (other.deleted - self.inserted)`
    ///
    /// The result is again a valid delta (its two sets are disjoint).
    ///
    /// # Precondition
    ///
    /// `self.compose(other).apply(r) == other.apply(&self.apply(r))` holds
    /// whenever `self` is effective for `r` ([`Delta::is_effective_for`]),
    /// for any `other`. This always holds when `self` was produced by
    /// [`Delta::between`] with `old == r`; for such chains
    /// `between(r1, r2).compose(&between(r2, r3)) == between(r1, r3)`.
    ///
    /// Without that precondition the composition is not well defined from
    /// the two deltas alone: e.g. a `self` that "inserts" a tuple already in
    /// `r`, followed by an `other` that deletes it, nets to "no change" here,
    /// whereas applying them in sequence removes the tuple.
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
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    fn test_type() -> RelationType {
        RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int))
    }

    #[test]
    fn test_delta_between_empty() {
        let t = test_type();
        let r1 = Relation::new(t.clone());
        let r2 = Relation::new(t.clone());

        let delta = Delta::between(&r1, &r2).unwrap();
        assert!(delta.inserted().is_empty());
        assert!(delta.deleted().is_empty());
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
        assert_eq!(delta.deleted().cardinality(), 1);
        assert!(delta.deleted().contains(&tuple! { x: 1i64 }));
        assert_eq!(delta.inserted().cardinality(), 1);
        assert!(delta.inserted().contains(&tuple! { x: 2i64 }));

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
        assert!(inv.deleted().contains(&tuple! { x: 2i64 }));
        assert!(inv.inserted().contains(&tuple! { x: 1i64 }));

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

        assert!(d3.deleted().contains(&tuple! { x: 1i64 }));
        assert!(!d3.deleted().contains(&tuple! { x: 2i64 })); // 2 was ins then del, so net zero

        assert!(d3.inserted().contains(&tuple! { x: 3i64 }));
        assert!(!d3.inserted().contains(&tuple! { x: 2i64 })); // 2 was ins then del, so net zero

        let r_final = d3.apply(&r1).unwrap();
        assert_eq!(r_final, r3);
    }

    /// A tuple cannot be both inserted and deleted: `Delta::new` must reject
    /// overlapping sets (otherwise `apply` silently lets the insert win).
    #[test]
    fn test_delta_new_rejects_overlapping_inserted_and_deleted() {
        let t = test_type();
        let mut inserted = Relation::new(t.clone());
        inserted.insert(tuple! { x: 1i64 }).unwrap();
        inserted.insert(tuple! { x: 2i64 }).unwrap();
        let mut deleted = Relation::new(t);
        deleted.insert(tuple! { x: 2i64 }).unwrap();

        let result = Delta::new(inserted, deleted);
        assert!(
            matches!(result, Err(DatabaseError::AlgebraError(ref msg)) if msg.contains("both")),
            "expected overlap rejection, got {result:?}"
        );
    }

    /// Composition is exact whenever `self` is effective for the base it is
    /// applied to, even if `other` is not minimal for the intermediate state.
    #[test]
    fn test_compose_with_non_minimal_second_delta() {
        let t = test_type();
        // R1 = {1}; d1 = between(R1, {2}): delete 1, insert 2 (effective).
        let mut r1 = Relation::new(t.clone());
        r1.insert(tuple! { x: 1i64 }).unwrap();
        let mut r2 = Relation::new(t.clone());
        r2.insert(tuple! { x: 2i64 }).unwrap();
        let d1 = Delta::between(&r1, &r2).unwrap();
        assert!(d1.is_effective_for(&r1));

        // d2 re-inserts 2 (already present) and deletes 3 (absent).
        let mut ins = Relation::new(t.clone());
        ins.insert(tuple! { x: 2i64 }).unwrap();
        let mut del = Relation::new(t);
        del.insert(tuple! { x: 3i64 }).unwrap();
        let d2 = Delta::new(ins, del).unwrap();
        assert!(!d2.is_effective_for(&r2));

        let composed = d1.compose(&d2).unwrap();
        assert_eq!(
            composed.apply(&r1).unwrap(),
            d2.apply(&d1.apply(&r1).unwrap()).unwrap()
        );
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

        assert_eq!(d3.inserted().cardinality(), 2);
        assert!(d3.inserted().contains(&tuple! { x: 1i64 }));
        assert!(d3.inserted().contains(&tuple! { x: 2i64 }));

        let r_final = d3.apply(&r1).unwrap();
        assert_eq!(r_final, r3);
    }
    fn test_type_2() -> RelationType {
        RelationType::new(TupleType::new().with_attribute("y", ScalarType::Int))
    }

    #[test]
    fn test_delta_new_type_mismatch() {
        let r1 = Relation::new(test_type());
        let r2 = Relation::new(test_type_2());

        let result = Delta::new(r1, r2);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }

    #[test]
    fn test_delta_between_type_mismatch() {
        let r1 = Relation::new(test_type());
        let r2 = Relation::new(test_type_2());

        let result = Delta::between(&r1, &r2);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }

    #[test]
    fn test_delta_apply_type_mismatch() {
        let r1 = Relation::new(test_type());
        let r2 = Relation::new(test_type());
        let r3 = Relation::new(test_type_2());

        let delta = Delta::between(&r1, &r2).unwrap();
        let result = delta.apply(&r3);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }

    #[test]
    fn test_delta_compose_type_mismatch() {
        let r1 = Relation::new(test_type());
        let r2 = Relation::new(test_type());
        let r3 = Relation::new(test_type_2());
        let r4 = Relation::new(test_type_2());

        let d1 = Delta::between(&r1, &r2).unwrap();
        let d2 = Delta::between(&r3, &r4).unwrap();

        let result = d1.compose(&d2);
        assert!(matches!(result, Err(DatabaseError::AlgebraError(_))));
    }
}

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

#[cfg(test)]
mod compose_properties {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use alloc::collections::BTreeSet;
    use proptest::prelude::*;

    fn rel(xs: &BTreeSet<i64>) -> Relation {
        let mut r = Relation::new(RelationType::new(
            TupleType::new().with_attribute("x", ScalarType::Int),
        ));
        for x in xs {
            r.insert(tuple! { x: *x }).unwrap();
        }
        r
    }

    fn set() -> impl Strategy<Value = BTreeSet<i64>> {
        prop::collection::btree_set(0i64..8, 0..8)
    }

    proptest! {
        /// For deltas produced by `between`, composing then applying equals
        /// applying in sequence, and the composition is `between(r1, r3)`.
        #[test]
        fn compose_of_between_deltas_matches_sequential_apply(
            a in set(), b in set(), c in set()
        ) {
            let (r1, r2, r3) = (rel(&a), rel(&b), rel(&c));
            let d1 = Delta::between(&r1, &r2).unwrap();
            let d2 = Delta::between(&r2, &r3).unwrap();
            let composed = d1.compose(&d2).unwrap();
            prop_assert_eq!(
                composed.apply(&r1).unwrap(),
                d2.apply(&d1.apply(&r1).unwrap()).unwrap()
            );
            prop_assert_eq!(composed, Delta::between(&r1, &r3).unwrap());
        }

        /// Composition stays exact for an arbitrary (disjoint) second delta,
        /// as long as the first is effective for the base.
        #[test]
        fn compose_with_arbitrary_second_delta_matches_sequential_apply(
            a in set(), b in set(), ins in set(), del in set()
        ) {
            let (r1, r2) = (rel(&a), rel(&b));
            let d1 = Delta::between(&r1, &r2).unwrap();
            let del: BTreeSet<i64> = del.difference(&ins).copied().collect();
            let d2 = Delta::new(rel(&ins), rel(&del)).unwrap();
            let composed = d1.compose(&d2).unwrap();
            prop_assert!(composed.inserted().intersect(composed.deleted()).unwrap().is_empty());
            prop_assert_eq!(
                composed.apply(&r1).unwrap(),
                d2.apply(&d1.apply(&r1).unwrap()).unwrap()
            );
        }

        /// `Delta::new` accepts exactly the disjoint pairs.
        #[test]
        fn new_accepts_only_disjoint_sets(ins in set(), del in set()) {
            let result = Delta::new(rel(&ins), rel(&del));
            prop_assert_eq!(result.is_ok(), ins.is_disjoint(&del));
        }
    }
}
