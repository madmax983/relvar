//! Relational CRDTs (Conflict-free Replicated Data Types)
//!
//! This module demonstrates how to build simple CRDTs using purely relational
//! algebra operations.
//!
//! # Included CRDTs
//!
//! - **LwwMap**: Last-Write-Wins Map.

use relvar_core::{
    algebra::Aggregation,
    types::{RelationType, ScalarType, TupleType},
    values::Relation,
};

/// A Relational Last-Write-Wins (LWW) Map CRDT.
///
/// Models a key-value store where concurrent updates are resolved by timestamp.
/// The relation schema is `(key: String, value: String, timestamp: Int)`.
pub struct LwwMap;

impl LwwMap {
    /// Returns the relation schema for the LWW Map.
    pub fn schema() -> RelationType {
        RelationType::new(
            TupleType::new()
                .with_attribute("key", ScalarType::String)
                .with_attribute("value", ScalarType::String)
                .with_attribute("timestamp", ScalarType::Int),
        )
    }

    /// Merges two LWW Map replicas, keeping the latest value for each key.
    pub fn merge(replica_a: &Relation, replica_b: &Relation) -> Relation {
        let combined = replica_a.union(replica_b).unwrap();
        let max_timestamps = combined.summarize(
            &["key"],
            &[Aggregation::max("max_ts", "timestamp", ScalarType::Int)],
        ).unwrap();
        let max_renamed = max_timestamps.rename(&[("max_ts", "timestamp")]);
        combined.join(&max_renamed).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_lww_merge() {
        let schema = LwwMap::schema();
        let mut r1 = Relation::new(schema.clone());
        let mut r2 = Relation::new(schema.clone());

        r1.insert(tuple! {
            key: "color".to_string(),
            value: "red".to_string(),
            timestamp: 1i64
        }).unwrap();

        r2.insert(tuple! {
            key: "color".to_string(),
            value: "blue".to_string(),
            timestamp: 2i64
        }).unwrap();

        let merged = LwwMap::merge(&r1, &r2);
        assert_eq!(merged.cardinality(), 1);

        let t = merged.tuples().next().unwrap();
        assert_eq!(t.get_typed::<String>("value").unwrap(), "blue");
    }
}
