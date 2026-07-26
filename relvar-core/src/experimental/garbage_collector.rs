//! Experimental Relational Garbage Collector Implementation.

use crate::error::DatabaseError;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A Mark-and-Sweep Garbage Collector modeled using purely Relational Algebra.
pub struct RelationalGC {
    roots: Relation,
    heap: Relation,
    references: Relation,
}

impl Default for RelationalGC {
    fn default() -> Self {
        Self::new()
    }
}

impl RelationalGC {
    /// Creates a new empty Relational Garbage Collector state.
    pub fn new() -> Self {
        let id_type = TupleType::new().with_attribute("id", ScalarType::Int);
        let roots = Relation::new(RelationType::new(id_type.clone()));
        let heap = Relation::new(RelationType::new(id_type));

        let ref_type = TupleType::new()
            .with_attribute("from_id", ScalarType::Int)
            .with_attribute("to_id", ScalarType::Int);
        let references = Relation::new(RelationType::new(ref_type));

        Self {
            roots,
            heap,
            references,
        }
    }

    /// Allocates an object on the heap.
    pub fn allocate(&mut self, id: i64) -> Result<(), DatabaseError> {
        let _ = self.heap.insert(tuple! { id: id })?;
        Ok(())
    }

    /// Adds an object to the GC root set.
    pub fn add_root(&mut self, id: i64) -> Result<(), DatabaseError> {
        let _ = self.roots.insert(tuple! { id: id })?;
        Ok(())
    }

    /// Adds a reference from one object to another.
    pub fn add_reference(&mut self, from_id: i64, to_id: i64) -> Result<(), DatabaseError> {
        let _ = self
            .references
            .insert(tuple! { from_id: from_id, to_id: to_id })?;
        Ok(())
    }

    /// Computes the set of reachable objects (Mark phase).
    pub fn mark(&self) -> Result<Relation, DatabaseError> {
        // If there are no references, just return the roots
        if self.references.is_empty() {
            return Ok(self.roots.clone());
        }

        // Find reachability graph via Transitive Closure of references
        // `tclose` requires binary relation where attributes have the same type,
        // which `from_id` and `to_id` satisfy.
        let reachable_graph = self.references.tclose("from_id", "to_id")?;

        // Find nodes reachable from roots:
        // Rename root attribute 'id' to 'from_id' to join with the reachable graph
        let renamed_roots = self.roots.rename(&[("id", "from_id")]);

        // Join roots with reachable graph to get (from_id, to_id) where from_id is a root
        let root_to_reachable = renamed_roots.join(&reachable_graph)?;

        // Project to just the 'to_id' (reachable objects)
        let mut all_reachable = root_to_reachable.project(&["to_id"]);

        // Rename 'to_id' back to 'id' to match heap schema
        all_reachable = all_reachable.rename_into(&[("to_id", "id")]);

        // Ensure the roots themselves are included in the reachable set (union)
        let mark_set = all_reachable
            .union(&self.roots)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(mark_set)
    }

    /// Computes the set of unreachable objects (Sweep phase).
    pub fn sweep(&self) -> Result<Relation, DatabaseError> {
        // Sweep = Heap MINUS Mark Set
        let mark_set = self.mark()?;
        self.heap
            .difference(&mark_set)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_mark_and_sweep() {
        let mut gc = RelationalGC::new();

        // Heap: 1, 2, 3, 4, 5, 6
        for i in 1..=6 {
            gc.allocate(i).unwrap();
        }

        // Roots: 1
        gc.add_root(1).unwrap();

        // References:
        // 1 -> 2
        // 2 -> 3
        // 4 -> 5
        gc.add_reference(1, 2).unwrap();
        gc.add_reference(2, 3).unwrap();
        gc.add_reference(4, 5).unwrap();

        // Mark phase should find 1, 2, 3 as reachable
        let reachable = gc.mark().unwrap();
        assert_eq!(reachable.cardinality(), 3);
        assert!(reachable.contains(&tuple! { id: 1i64 }));
        assert!(reachable.contains(&tuple! { id: 2i64 }));
        assert!(reachable.contains(&tuple! { id: 3i64 }));

        // Sweep phase should identify 4, 5, 6 as garbage
        let garbage = gc.sweep().unwrap();
        assert_eq!(garbage.cardinality(), 3);
        assert!(garbage.contains(&tuple! { id: 4i64 }));
        assert!(garbage.contains(&tuple! { id: 5i64 }));
        assert!(garbage.contains(&tuple! { id: 6i64 }));
    }

    #[test]
    fn test_gc_no_references() {
        let mut gc = RelationalGC::new();
        gc.allocate(1).unwrap();
        gc.allocate(2).unwrap();
        gc.add_root(1).unwrap();

        let reachable = gc.mark().unwrap();
        assert_eq!(reachable.cardinality(), 1);

        let garbage = gc.sweep().unwrap();
        assert_eq!(garbage.cardinality(), 1);
        assert!(garbage.contains(&tuple! { id: 2i64 }));
    }
}
