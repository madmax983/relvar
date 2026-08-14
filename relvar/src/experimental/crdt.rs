//! Relational Conflict-free Replicated Data Types (CRDTs)
//!
//! This module demonstrates how distributed data structures like an
//! Observed-Remove Set (OR-Set) can be implemented using purely relational algebra.
//!
//! # Concept
//!
//! An OR-Set allows elements to be added and removed. In a distributed system,
//! concurrent adds and removes of the same element must be resolved. By tagging
//! every `add` with a unique tag (e.g., a UUID), a `remove` only removes the
//! specific tags it has "observed".
//!
//! - **Adds**: Relation `(element: String, tag: String)`.
//! - **Removes**: Relation `(element: String, tag: String)`.
//!
//! The operations map elegantly to relational algebra:
//! - **Read**: `(Adds MINUS Removes) PROJECT element`
//! - **Remove(e)**: `Removes = Removes UNION (Adds RESTRICT element == e)`
//! - **Merge(Other)**: `Adds = Adds UNION Other.Adds`, `Removes = Removes UNION Other.Removes`

use relvar_core::{
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Observed-Remove Set (OR-Set).
/// # Examples
///
/// ```
/// use relvar::experimental::crdt::OrSet;
///
/// let mut set1 = OrSet::new();
/// set1.add("apple", "tag1").unwrap();
/// set1.add("banana", "tag2").unwrap();
///
/// let mut set2 = OrSet::new();
/// set2.add("apple", "tag3").unwrap();
///
/// set1.merge(&set2).unwrap();
///
/// // Elements are "apple" and "banana"
/// let elements = set1.read().unwrap();
/// assert_eq!(elements.cardinality(), 2);
/// ```
pub struct OrSet {
    /// The add set. Schema: (element: String, tag: String)
    pub adds: Relation,
    /// The remove set. Schema: (element: String, tag: String)
    pub removes: Relation,
}

impl OrSet {
    /// Creates a new empty OR-Set.
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("element", ScalarType::String)
            .with_attribute("tag", ScalarType::String);
        let rel_type = RelationType::new(heading);

        Self {
            adds: Relation::new(rel_type.clone()),
            removes: Relation::new(rel_type),
        }
    }

    /// Adds an element with a unique tag.
    pub fn add(&mut self, element: &str, tag: &str) -> Result<(), DatabaseError> {
        let mut vals = std::collections::BTreeMap::new();
        vals.insert(
            "element".to_string(),
            ScalarValue::String(element.to_string()),
        );
        vals.insert("tag".to_string(), ScalarValue::String(tag.to_string()));

        let tuple = Tuple::new(self.adds.relation_type().heading().clone(), vals)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        self.adds.insert(tuple)?;
        Ok(())
    }

    /// Removes an element by "observing" all current tags for it.
    pub fn remove(&mut self, element: &str) -> Result<(), DatabaseError> {
        let element_val = ScalarValue::String(element.to_string());

        let observed_adds = self
            .adds
            .restrict(|t| t.get("element") == Some(&element_val));

        self.removes = self
            .removes
            .union(&observed_adds)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(())
    }

    /// Returns the current active elements in the set.
    /// Evaluated by: (adds MINUS removes) PROJECT element.
    pub fn read(&self) -> Result<Relation, DatabaseError> {
        let active = self
            .adds
            .difference(&self.removes)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(active.project(&["element"]))
    }

    /// Merges another OR-Set into this one.
    pub fn merge(&mut self, other: &OrSet) -> Result<(), DatabaseError> {
        self.adds = self
            .adds
            .union(&other.adds)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        self.removes = self
            .removes
            .union(&other.removes)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(())
    }
}

impl Default for OrSet {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orset_basic() {
        let mut set = OrSet::new();

        set.add("A", "tag1").unwrap();
        set.add("B", "tag2").unwrap();

        let elements = set.read().unwrap();
        assert_eq!(elements.cardinality(), 2);

        set.remove("A").unwrap();

        let elements2 = set.read().unwrap();
        assert_eq!(elements2.cardinality(), 1);

        // Re-adding A with a new tag
        set.add("A", "tag3").unwrap();
        let elements3 = set.read().unwrap();
        assert_eq!(elements3.cardinality(), 2);
    }

    #[test]
    fn test_orset_merge_concurrent_add_remove() {
        let mut replica1 = OrSet::new();
        let mut replica2 = OrSet::new();

        replica1.add("A", "tag1").unwrap();
        replica2.merge(&replica1).unwrap();

        // Concurrent operations:
        // Replica 1 removes A
        replica1.remove("A").unwrap();

        // Replica 2 adds A again (or concurrent add of A)
        replica2.add("A", "tag2").unwrap();

        replica1.merge(&replica2).unwrap();

        // Since Replica 2 added a new tag, A should be present
        let elements = replica1.read().unwrap();
        assert_eq!(elements.cardinality(), 1);
        let t = elements.tuples().next().unwrap();
        assert_eq!(t.get_typed::<String>("element").unwrap(), "A");
    }
}
