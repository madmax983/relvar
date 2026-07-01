//! Relational Conflict-Free Replicated Data Types (CRDTs)
//!
//! This module implements common CRDTs using pure relational algebra.
//! It demonstrates how distributed data structures can be seamlessly
//! modeled as relations, with merge operations compiling down to simple
//! relational unions and state evaluation handled via difference and aggregation.
//!
//! # Included CRDTs
//!
//! - **ORSet**: Observed-Remove Set
//! - **LWWRegister**: Last-Writer-Wins Register

use relvar_core::tuple;
use relvar_core::{
    algebra::UnionError,
    algebra::{Aggregation, AggregationFn},
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue},
};

/// A Relational Observed-Remove Set (OR-Set).
///
/// An OR-Set allows elements to be added and removed. It handles concurrent
/// add and remove operations by tagging each added element with a unique tag.
/// A remove operation removes all observed tags for an element.
///
/// # Examples
///
/// ```
/// use relvar_core::values::Relation;
/// use relvar::experimental::crdt::ORSet;
///
/// let mut replica_a = ORSet::new();
/// let mut replica_b = ORSet::new();
///
/// replica_a.add("apple", 1).unwrap();
/// replica_b.add("banana", 2).unwrap();
/// replica_a.remove("apple").unwrap();
/// replica_a.add("apple", 3).unwrap();
///
/// replica_a.merge(&replica_b).unwrap();
///
/// let result = replica_a.evaluate();
/// assert_eq!(result.cardinality(), 2);
/// ```
pub struct ORSet {
    /// Relation storing added elements and their tags. Schema: (element: String, tag: Int)
    pub adds: Relation,
    /// Relation storing removed elements and their tags. Schema: (element: String, tag: Int)
    pub removes: Relation,
}

impl Default for ORSet {
    fn default() -> Self {
        Self::new()
    }
}

impl ORSet {
    /// Creates a new, empty ORSet.
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("element".to_string(), ScalarType::String)
            .with_attribute("tag".to_string(), ScalarType::Int);
        let rel_type = RelationType::new(heading);

        Self {
            adds: Relation::new(rel_type.clone()),
            removes: Relation::new(rel_type),
        }
    }

    /// Adds an element to the set with a unique tag.
    pub fn add(&mut self, element: &str, tag: i64) -> Result<(), DatabaseError> {
        self.adds.insert(tuple! {
            element: element.to_string(),
            tag: tag
        })?;
        Ok(())
    }

    /// Removes an element from the set by observing current tags.
    pub fn remove(&mut self, element: &str) -> Result<(), UnionError> {
        let current_tags = self.adds.restrict(|t| {
            if let Some(ScalarValue::String(e)) = t.get("element") {
                e == element
            } else {
                false
            }
        });

        self.removes = self.removes.union(&current_tags)?;
        Ok(())
    }

    /// Merges another ORSet into this one.
    pub fn merge(&mut self, other: &ORSet) -> Result<(), UnionError> {
        self.adds = self.adds.union(&other.adds)?;
        self.removes = self.removes.union(&other.removes)?;
        Ok(())
    }

    /// Evaluates the current state of the set.
    pub fn evaluate(&self) -> Relation {
        let active = self
            .adds
            .difference(&self.removes)
            .unwrap_or_else(|_| self.adds.clone());
        active.project(&["element"]) // project returns a Relation directly
    }
}

/// A Relational Last-Writer-Wins (LWW) Register.
///
/// An LWW-Register stores a key-value pair along with a timestamp. When merging,
/// the entry with the highest timestamp wins. If timestamps are equal, a determinisitic
/// fallback should theoretically be used (though omitted for simplicity here).
///
/// # Examples
///
/// ```
/// use relvar::experimental::crdt::LWWRegister;
///
/// let mut reg_a = LWWRegister::new();
/// let mut reg_b = LWWRegister::new();
///
/// reg_a.write("status", "active", 10).unwrap();
/// reg_b.write("status", "inactive", 20).unwrap();
///
/// reg_a.merge(&reg_b).unwrap();
/// let current = reg_a.evaluate();
/// assert_eq!(current.cardinality(), 1);
/// ```
pub struct LWWRegister {
    /// Relation storing all writes. Schema: (key: String, value: String, timestamp: Int)
    pub entries: Relation,
}

impl Default for LWWRegister {
    fn default() -> Self {
        Self::new()
    }
}

impl LWWRegister {
    /// Creates a new, empty LWWRegister.
    pub fn new() -> Self {
        let heading = TupleType::new()
            .with_attribute("key".to_string(), ScalarType::String)
            .with_attribute("value".to_string(), ScalarType::String)
            .with_attribute("timestamp".to_string(), ScalarType::Int);
        let rel_type = RelationType::new(heading);

        Self {
            entries: Relation::new(rel_type),
        }
    }

    /// Writes a value to the register with a given timestamp.
    pub fn write(&mut self, key: &str, value: &str, timestamp: i64) -> Result<(), DatabaseError> {
        self.entries.insert(tuple! {
            key: key.to_string(),
            value: value.to_string(),
            timestamp: timestamp
        })?;
        Ok(())
    }

    /// Merges another LWWRegister into this one.
    pub fn merge(&mut self, other: &LWWRegister) -> Result<(), UnionError> {
        self.entries = self.entries.union(&other.entries)?;
        Ok(())
    }

    /// Evaluates the current state of the register by finding the max timestamp for each key.
    pub fn evaluate(&self) -> Relation {
        // Find the maximum timestamp per key
        let max_ts = self
            .entries
            .summarize(
                &["key"],
                &[Aggregation {
                    result_name: "timestamp".to_string(),
                    result_type: ScalarType::Int,
                    function: AggregationFn::Max("timestamp".to_string()),
                }],
            )
            .unwrap_or_else(|_| self.entries.clone());

        // Join back to get the actual values
        self.entries
            .join(&max_ts)
            .unwrap_or_else(|_| self.entries.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orset_merge() {
        let mut replica_a = ORSet::new();
        let mut replica_b = ORSet::new();

        replica_a.add("apple", 1).unwrap();
        replica_b.add("banana", 2).unwrap();

        replica_a.remove("apple").unwrap();
        replica_a.add("apple", 3).unwrap();

        replica_a.merge(&replica_b).unwrap();

        let result = replica_a.evaluate();
        assert_eq!(result.cardinality(), 2);
    }

    #[test]
    fn test_lww_register_merge() {
        let mut reg_a = LWWRegister::new();
        let mut reg_b = LWWRegister::new();

        reg_a.write("theme", "light", 100).unwrap();
        reg_b.write("theme", "dark", 150).unwrap();

        reg_a.merge(&reg_b).unwrap();
        let result = reg_a.evaluate();

        assert_eq!(result.cardinality(), 1);
        for t in result.tuples() {
            assert_eq!(
                t.get("value").unwrap(),
                &ScalarValue::String("dark".to_string())
            );
            assert_eq!(
                t.get("timestamp").unwrap(),
                &ScalarValue::Int(150)
            );
        }
    }
}
