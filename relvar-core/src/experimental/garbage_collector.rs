//! Experimental Relational Garbage Collector.
//!
//! This module models a Mark-and-Sweep Garbage Collector purely with relational algebra.
//! It uses relations to represent the set of root pointers, the allocated heap,
//! and the reference edges between objects.
//!
//! Reachability is computed using the `tclose` operator.
//! Unreachable objects (garbage) are identified via relational difference.

use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A simple Relational Garbage Collector model.
pub struct GarbageCollector {
    /// R(id: Int) - the roots.
    pub roots: Relation,
    /// R(id: Int) - the set of all allocated objects.
    pub heap: Relation,
    /// R(from: Int, to: Int) - object references.
    pub references: Relation,
}

impl GarbageCollector {
    /// Creates a new, empty Garbage Collector instance.
    pub fn new() -> Self {
        let id_type = TupleType::new().with_attribute("id", ScalarType::Int);
        let ref_type = TupleType::new()
            .with_attribute("from", ScalarType::Int)
            .with_attribute("to", ScalarType::Int);

        Self {
            roots: Relation::new(RelationType::new(id_type.clone())),
            heap: Relation::new(RelationType::new(id_type)),
            references: Relation::new(RelationType::new(ref_type)),
        }
    }

    /// Evaluates the mark-and-sweep algorithm to find garbage objects.
    ///
    /// # Returns
    ///
    /// A relation containing the `id` of all unreachable objects.
    pub fn collect_garbage(&self) -> Result<Relation, DatabaseError> {
        // Find all transitively reachable references: R(from: Int, to: Int)
        // If there are no references, the closure is just empty.
        let closure = if self.references.cardinality() > 0 {
            self.references.tclose("from", "to")?
        } else {
            self.references.clone()
        };

        // Find objects reachable from roots
        // We join `roots(id)` with `closure(from, to)`.
        // To do this, we temporarily rename `closure.from` to `id`, then project `to`, and rename `to` to `id`.
        // Wait, natural join needs matching attributes.
        // roots: (id)
        // closure: (from, to)
        // rename roots(id -> from)
        let roots_from = self.roots.rename(&[("id", "from")]);
        let reachable_from_roots_edges = closure.join(&roots_from)?;
        let reachable_from_roots_to = reachable_from_roots_edges.project(&["to"]);
        let mut reachable = reachable_from_roots_to.rename(&[("to", "id")]);

        // Note: roots themselves are reachable!
        // So reachable = reachable UNION roots
        reachable = reachable.union(&self.roots).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Find garbage: heap MINUS reachable
        self.heap.difference(&reachable).map_err(|e| DatabaseError::AlgebraError(e.to_string()))
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
    use crate::tuple;

    #[test]
    fn test_garbage_collector() -> Result<(), DatabaseError> {
        let mut gc = GarbageCollector::new();

        // 1. Setup Heap
        gc.heap.insert(tuple! { id: 1i64 })?; // Root
        gc.heap.insert(tuple! { id: 2i64 })?; // Reachable from 1
        gc.heap.insert(tuple! { id: 3i64 })?; // Reachable from 2
        gc.heap.insert(tuple! { id: 4i64 })?; // Unreachable
        gc.heap.insert(tuple! { id: 5i64 })?; // Unreachable, cyclic with 6
        gc.heap.insert(tuple! { id: 6i64 })?; // Unreachable, cyclic with 5

        // 2. Setup Roots
        gc.roots.insert(tuple! { id: 1i64 })?;

        // 3. Setup References
        gc.references.insert(tuple! { from: 1i64, to: 2i64 })?;
        gc.references.insert(tuple! { from: 2i64, to: 3i64 })?;
        gc.references.insert(tuple! { from: 5i64, to: 6i64 })?;
        gc.references.insert(tuple! { from: 6i64, to: 5i64 })?;

        // 4. Collect Garbage
        let garbage = gc.collect_garbage()?;

        // Expected garbage: 4, 5, 6
        assert_eq!(garbage.cardinality(), 3);
        assert!(garbage.contains(&tuple! { id: 4i64 }));
        assert!(garbage.contains(&tuple! { id: 5i64 }));
        assert!(garbage.contains(&tuple! { id: 6i64 }));

        Ok(())
    }

    #[test]
    fn test_empty_references() -> Result<(), DatabaseError> {
        let mut gc = GarbageCollector::new();
        gc.heap.insert(tuple! { id: 1i64 })?;
        gc.roots.insert(tuple! { id: 1i64 })?;
        gc.heap.insert(tuple! { id: 2i64 })?; // Garbage

        let garbage = gc.collect_garbage()?;
        assert_eq!(garbage.cardinality(), 1);
        assert!(garbage.contains(&tuple! { id: 2i64 }));

        Ok(())
    }
}
