//! Relational Conflict-free Replicated Data Type (CRDT)
//!
//! This module demonstrates how to implement an Observed-Remove Set (OR-Set)
//! using purely relational algebra.
//!
//! # Concept
//!
//! An OR-Set allows distributed addition and removal of elements without coordination.
//! It uses two relations: `adds` and `removes`.
//!
//! - **adds**: `(element, tag)`
//! - **removes**: `(element, tag)`
//!
//! A tag is a unique identifier generated upon addition.
//! To add an element, a new `(element, tag)` tuple is inserted into `adds`.
//! To remove an element, all currently observed `(element, tag)` tuples for that element
//! are inserted into `removes`.
//!
//! The current members of the set are `project(element, adds - removes)`.
//! Merging two replicas is simply taking the union of their respective `adds` and `removes` relations.

use relvar_core::{
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Observed-Remove Set (OR-Set) CRDT.
///
/// # Examples
///
/// ```
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar::experimental::crdt::RelationalORSet;
///
/// let element_type = ScalarType::String;
/// let set1 = RelationalORSet::new(element_type.clone());
/// ```
pub struct RelationalORSet {
    /// Schema: (element: T, tag: String)
    pub adds: Relation,
    /// Schema: (element: T, tag: String)
    pub removes: Relation,
    /// The type of the element
    _element_type: ScalarType,
}

impl RelationalORSet {
    /// Creates a new, empty OR-Set for the given element type.
    pub fn new(element_type: ScalarType) -> Self {
        let heading = TupleType::new()
            .with_attribute("element", element_type.clone())
            .with_attribute("tag", ScalarType::String);
        let rel_type = RelationType::new(heading);

        Self {
            adds: Relation::new(rel_type.clone()),
            removes: Relation::new(rel_type),
            _element_type: element_type,
        }
    }

    /// Adds a new element to the set with a unique tag.
    pub fn add(&mut self, element: ScalarValue, tag: String) -> Result<(), DatabaseError> {
        let heading = self.adds.relation_type().heading().clone();
        let mut tuple_values = std::collections::HashMap::new();
        tuple_values.insert("element".to_string(), element);
        tuple_values.insert("tag".to_string(), ScalarValue::String(tag));
        let tuple = Tuple::new(heading, tuple_values)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        self.adds.insert(tuple)?;
        Ok(())
    }

    /// Removes an element by adding all currently observed tags for it to the removes set.
    pub fn remove(&mut self, element: ScalarValue) -> Result<(), DatabaseError> {
        // Restrict adds to the element being removed
        let target_element = element.clone();
        let observed_adds = self
            .adds
            .restrict(move |t| t.get("element") == Some(&target_element));

        // Add all observed additions to the removes set
        for tuple in observed_adds.tuples() {
            self.removes.insert(tuple.clone())?;
        }

        Ok(())
    }

    /// Returns a relation of the current members.
    /// Schema: (element: T)
    pub fn members(&self) -> Result<Relation, DatabaseError> {
        let active = self
            .adds
            .difference(&self.removes)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(active.project(&["element"]))
    }

    /// Merges another OR-Set into this one.
    pub fn merge(&mut self, other: &RelationalORSet) -> Result<(), DatabaseError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orset_basic_operations() {
        let mut set = RelationalORSet::new(ScalarType::String);

        // Add "apple"
        set.add(ScalarValue::String("apple".to_string()), "tag1".to_string())
            .unwrap();

        let members = set.members().unwrap();
        assert_eq!(members.cardinality(), 1);

        // Remove "apple"
        set.remove(ScalarValue::String("apple".to_string()))
            .unwrap();

        let members2 = set.members().unwrap();
        assert_eq!(members2.cardinality(), 0);

        // Add "apple" again with a new tag
        set.add(ScalarValue::String("apple".to_string()), "tag2".to_string())
            .unwrap();

        let members3 = set.members().unwrap();
        assert_eq!(members3.cardinality(), 1);
    }

    #[test]
    fn test_orset_merge() {
        let mut replica1 = RelationalORSet::new(ScalarType::String);
        let mut replica2 = RelationalORSet::new(ScalarType::String);

        // Replica 1 adds "apple"
        replica1
            .add(
                ScalarValue::String("apple".to_string()),
                "tag_A".to_string(),
            )
            .unwrap();

        // Replica 2 adds "banana"
        replica2
            .add(
                ScalarValue::String("banana".to_string()),
                "tag_B".to_string(),
            )
            .unwrap();

        // Replicas merge
        replica1.merge(&replica2).unwrap();
        replica2.merge(&replica1).unwrap();

        assert_eq!(replica1.members().unwrap().cardinality(), 2);
        assert_eq!(replica2.members().unwrap().cardinality(), 2);

        // Replica 1 removes "apple"
        replica1
            .remove(ScalarValue::String("apple".to_string()))
            .unwrap();

        // Replica 2 doesn't know about the remove yet.
        assert_eq!(replica1.members().unwrap().cardinality(), 1); // Only "banana"
        assert_eq!(replica2.members().unwrap().cardinality(), 2); // "apple" and "banana"

        // Replica 2 concurrently adds "apple" again (new tag!)
        replica2
            .add(
                ScalarValue::String("apple".to_string()),
                "tag_C".to_string(),
            )
            .unwrap();

        // Merge again
        replica1.merge(&replica2).unwrap();

        // "apple" should be present because of the new tag "tag_C" which was not removed.
        let members = replica1.members().unwrap();
        assert_eq!(members.cardinality(), 2);
    }
}
