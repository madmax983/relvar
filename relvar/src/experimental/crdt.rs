//! Relational Conflict-Free Replicated Data Types (CRDTs).
//!
//! This module demonstrates how Conflict-Free Replicated Data Types, specifically
//! an Observed-Remove Set (OR-Set), can be implemented using purely relational
//! algebra operations.
//!
//! # Concept
//!
//! An OR-Set allows adding and removing elements concurrently without coordination.
//! It achieves this by attaching unique tags to each element insertion.
//!
//! We represent the internal state of an OR-Set using two relations:
//! - **Adds**: `(element: String, tag: String)`. Records all insertions.
//! - **Removes**: `(element: String, tag: String)`. Records all removals (tombstones).
//!
//! An element is considered present in the set if it exists in `Adds` but
//! its specific tag does not exist in `Removes`.
//!
//! The state of the set is derived purely via relational difference and projection:
//! `Project(Adds MINUS Removes, [element])`
//!
//! Merging two OR-Sets is simply taking the relational Union of their respective
//! `Adds` and `Removes` relations.

use relvar_core::{error::DatabaseError, values::Relation};

/// A Relational Observed-Remove Set (OR-Set) CRDT.
///
/// # Examples
///
/// ```
/// use relvar_core::{tuple, Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::crdt::OrSet;
///
/// let schema = TupleType::new()
///     .with_attribute("element", ScalarType::String)
///     .with_attribute("tag", ScalarType::String);
///
/// let mut adds = Relation::new(RelationType::new(schema.clone()));
/// let mut removes = Relation::new(RelationType::new(schema.clone()));
///
/// // Node A adds "apple" with tag "t1"
/// adds.insert(tuple! { element: "apple", tag: "t1" }).unwrap();
///
/// let mut crdt = OrSet::new(adds, removes);
///
/// // The value is present
/// let elements = crdt.elements().unwrap();
/// assert_eq!(elements.cardinality(), 1);
/// ```
pub struct OrSet {
    /// The relation storing added elements and their unique tags.
    /// Schema: `(element: String, tag: String)`
    pub adds: Relation,
    /// The relation storing removed elements and the tags that were observed.
    /// Schema: `(element: String, tag: String)`
    pub removes: Relation,
}

impl OrSet {
    /// Creates a new OR-Set from the given `adds` and `removes` relations.
    ///
    /// The relations must have the schema `(element: String, tag: String)`.
    pub fn new(adds: Relation, removes: Relation) -> Self {
        Self { adds, removes }
    }

    /// Computes the current elements present in the set.
    ///
    /// An element is present if it is in `adds` but the exact `(element, tag)`
    /// pair is not in `removes`.
    ///
    /// Returns a relation with the schema `(element: String)`.
    pub fn elements(&self) -> Result<Relation, DatabaseError> {
        let active = self
            .adds
            .difference(&self.removes)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(active.project(&["element"]))
    }

    /// Merges another OR-Set into this one.
    ///
    /// The merge operation takes the relational Union of both `adds` sets
    /// and both `removes` sets. This is commutative, associative, and idempotent.
    pub fn merge_with(&mut self, other: &Self) -> Result<(), DatabaseError> {
        let new_adds = self
            .adds
            .union(&other.adds)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let new_removes = self
            .removes
            .union(&other.removes)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        self.adds = new_adds;
        self.removes = new_removes;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{
        tuple,
        types::{RelationType, ScalarType, TupleType},
    };

    fn empty_relation() -> Relation {
        let schema = TupleType::new()
            .with_attribute("element", ScalarType::String)
            .with_attribute("tag", ScalarType::String);
        Relation::new(RelationType::new(schema))
    }

    #[test]
    fn test_orset_basic() {
        let mut adds = empty_relation();
        let removes = empty_relation();

        adds.insert(tuple! { element: "A", tag: "1" }).unwrap();
        adds.insert(tuple! { element: "B", tag: "2" }).unwrap();

        let set = OrSet::new(adds, removes);
        let elements = set.elements().unwrap();

        assert_eq!(elements.cardinality(), 2);
    }

    #[test]
    fn test_orset_remove() {
        let mut adds = empty_relation();
        let mut removes = empty_relation();

        // Add A and B
        adds.insert(tuple! { element: "A", tag: "1" }).unwrap();
        adds.insert(tuple! { element: "B", tag: "2" }).unwrap();

        // Remove A (observing tag 1)
        removes.insert(tuple! { element: "A", tag: "1" }).unwrap();

        let set = OrSet::new(adds, removes);
        let elements = set.elements().unwrap();

        // Only B remains
        assert_eq!(elements.cardinality(), 1);
        let first_tuple = elements.tuples().next().unwrap();
        assert_eq!(first_tuple.get_typed::<String>("element").unwrap(), "B");
    }

    #[test]
    fn test_orset_merge_concurrent_add_remove() {
        let mut adds1 = empty_relation();
        let mut removes1 = empty_relation();

        // Node 1: Add A (tag 1)
        adds1.insert(tuple! { element: "A", tag: "1" }).unwrap();

        let mut adds2 = adds1.clone();
        let removes2 = removes1.clone();

        // Node 1: Removes A
        removes1.insert(tuple! { element: "A", tag: "1" }).unwrap();

        // Node 2 (concurrently): Adds A again (new tag 2)
        adds2.insert(tuple! { element: "A", tag: "2" }).unwrap();

        let mut set1 = OrSet::new(adds1, removes1);
        let set2 = OrSet::new(adds2, removes2);

        // Merge Node 2 state into Node 1
        set1.merge_with(&set2).unwrap();

        // Because Node 2's addition had a new tag not seen by Node 1's remove, A is present
        let elements = set1.elements().unwrap();
        assert_eq!(elements.cardinality(), 1);
    }
}
