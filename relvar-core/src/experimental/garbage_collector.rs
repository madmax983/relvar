//! Mark-and-Sweep Garbage Collector in Relational Algebra.
//!
//! This module implements a basic Mark-and-Sweep Garbage Collector purely
//! using relational algebra primitives.

use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A Garbage Collector modeled purely using relational algebra.
pub struct GarbageCollector {
    /// The set of all allocated objects. Heading: `{ id: Int }`
    pub heap: Relation,
    /// The set of root references (e.g., stack, globals). Heading: `{ id: Int }`
    pub roots: Relation,
    /// The set of references between objects. Heading: `{ from_id: Int, to_id: Int }`
    pub references: Relation,
}

impl GarbageCollector {
    /// Bootstraps an empty Garbage Collector state.
    pub fn new() -> Self {
        let id_type = TupleType::new().with_attribute("id", ScalarType::Int);
        let ref_type = TupleType::new()
            .with_attribute("from_id", ScalarType::Int)
            .with_attribute("to_id", ScalarType::Int);

        Self {
            heap: Relation::new(RelationType::new(id_type.clone())),
            roots: Relation::new(RelationType::new(id_type)),
            references: Relation::new(RelationType::new(ref_type)),
        }
    }

    /// Allocates an object on the heap.
    pub fn alloc(&mut self, id: i64) -> Result<(), DatabaseError> {
        let t = crate::tuple! { id: id };
        let _ = self
            .heap
            .insert(t)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Adds an object to the root set.
    pub fn add_root(&mut self, id: i64) -> Result<(), DatabaseError> {
        let t = crate::tuple! { id: id };
        let _ = self
            .roots
            .insert(t)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Adds a reference from one object to another.
    pub fn add_reference(&mut self, from_id: i64, to_id: i64) -> Result<(), DatabaseError> {
        let t = crate::tuple! { from_id: from_id, to_id: to_id };
        let _ = self
            .references
            .insert(t)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Computes the set of live objects via the Mark phase.
    pub fn mark(&self) -> Result<Relation, DatabaseError> {
        // If there are no references, tclose will return an empty relation of the same type,
        // or an error if there are no tuples? Wait, `tclose` on an empty relation is fine.
        let closure = self.references.tclose("from_id", "to_id")?;

        let roots_as_from = self.roots.rename(&[("id", "from_id")]);
        let reachable_edges = roots_as_from.join(&closure)?;

        let reachable = reachable_edges
            .project(&["to_id"])
            .rename(&[("to_id", "id")]);

        let live_objects = self
            .roots
            .union(&reachable)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(live_objects)
    }

    /// Computes the set of garbage objects via the Sweep phase.
    pub fn sweep(&self) -> Result<Relation, DatabaseError> {
        let live_objects = self.mark()?;
        let garbage = self
            .heap
            .difference(&live_objects)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(garbage)
    }
}

impl Default for GarbageCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_garbage_collection() {
        let mut gc = GarbageCollector::new();

        // Allocate some objects
        gc.alloc(1).unwrap();
        gc.alloc(2).unwrap();
        gc.alloc(3).unwrap();
        gc.alloc(4).unwrap();
        gc.alloc(5).unwrap();

        // Root is 1
        gc.add_root(1).unwrap();

        // 1 -> 2 -> 3 (Live chain)
        gc.add_reference(1, 2).unwrap();
        gc.add_reference(2, 3).unwrap();

        // 4 -> 5 (Garbage cycle/chain)
        gc.add_reference(4, 5).unwrap();
        gc.add_reference(5, 4).unwrap();

        let live = gc.mark().unwrap();
        assert_eq!(live.cardinality(), 3); // 1, 2, 3

        let garbage = gc.sweep().unwrap();
        assert_eq!(garbage.cardinality(), 2); // 4, 5

        assert!(garbage.contains(&crate::tuple! {id: 4i64}));
        assert!(garbage.contains(&crate::tuple! {id: 5i64}));
    }
}
