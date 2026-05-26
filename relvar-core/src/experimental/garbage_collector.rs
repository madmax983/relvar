//! Relational Garbage Collector Simulation.
//!
//! This module models a Mark-and-Sweep Garbage Collector using purely relational algebra.

use crate::error::DatabaseError;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A relational mark-and-sweep garbage collector simulator.
pub struct MarkAndSweepGC {
    /// The relation containing root objects.
    pub roots: Relation,
    /// The relation containing all allocated heap objects.
    pub heap: Relation,
    /// The relation containing all references (edges) between objects.
    pub references: Relation,
}

impl Default for MarkAndSweepGC {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkAndSweepGC {
    /// Creates a new garbage collector instance with empty relations.
    pub fn new() -> Self {
        let obj_type =
            RelationType::new(TupleType::new().with_attribute("obj_id", ScalarType::Int));
        let ref_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_obj", ScalarType::Int)
                .with_attribute("to_obj", ScalarType::Int),
        );

        Self {
            roots: Relation::new(obj_type.clone()),
            heap: Relation::new(obj_type),
            references: Relation::new(ref_type),
        }
    }

    /// Adds a root object reference.
    pub fn add_root(&mut self, obj_id: i64) -> Result<(), DatabaseError> {
        self.roots.insert(tuple! { obj_id: obj_id })?;
        Ok(())
    }

    /// Adds an allocated object to the heap.
    pub fn add_to_heap(&mut self, obj_id: i64) -> Result<(), DatabaseError> {
        self.heap.insert(tuple! { obj_id: obj_id })?;
        Ok(())
    }

    /// Adds a reference from one object to another.
    pub fn add_reference(&mut self, from_obj: i64, to_obj: i64) -> Result<(), DatabaseError> {
        self.references
            .insert(tuple! { from_obj: from_obj, to_obj: to_obj })?;
        Ok(())
    }

    /// Evaluates the Mark phase of the GC, returning all reachable objects
    pub fn mark(&self) -> Result<Relation, DatabaseError> {
        // 1. Compute all transitive paths in the reference graph
        let paths = self.references.tclose("from_obj", "to_obj")?;

        // 2. Find all reachable objects from roots
        let reachable_from_roots = self
            .roots
            .clone()
            .rename(&[("obj_id", "from_obj")])
            .join(&paths)?
            .project(&["to_obj"])
            .rename(&[("to_obj", "obj_id")]);

        // 3. Roots are also reachable
        let marked = self
            .roots
            .clone()
            .union(&reachable_from_roots)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(marked)
    }

    /// Evaluates the Sweep phase, returning garbage objects
    pub fn sweep(&self) -> Result<Relation, DatabaseError> {
        let marked = self.mark()?;
        let garbage = self
            .heap
            .clone()
            .difference(&marked)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(garbage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mark_and_sweep() -> Result<(), DatabaseError> {
        let mut gc = MarkAndSweepGC::new();

        // Heap objects: 1, 2, 3, 4, 5, 6
        for id in 1..=6 {
            gc.add_to_heap(id)?;
        }

        // Roots: 1, 4
        gc.add_root(1)?;
        gc.add_root(4)?;

        // References:
        // 1 -> 2
        // 2 -> 3
        // 4 -> 4 (self ref)
        // 5 -> 6 (island)
        gc.add_reference(1, 2)?;
        gc.add_reference(2, 3)?;
        gc.add_reference(4, 4)?;
        gc.add_reference(5, 6)?;

        // Mark should find 1, 2, 3, 4
        let marked = gc.mark()?;
        assert_eq!(marked.cardinality(), 4);

        // Sweep should find 5, 6
        let garbage = gc.sweep()?;
        assert_eq!(garbage.cardinality(), 2);

        Ok(())
    }
}
