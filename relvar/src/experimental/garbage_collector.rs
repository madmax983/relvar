//! Relational Garbage Collector
//!
//! Models a Mark-and-Sweep Garbage Collector using purely relational algebra.
//!
//! # Concept
//!
//! - **Roots**: Relation `(address: Int)`. Represents root pointers (e.g., from stack or registers).
//! - **Heap**: Relation `(address: Int, size: Int)`. Represents all allocated objects in memory.
//! - **References**: Relation `(from_addr: Int, to_addr: Int)`. Represents pointers between objects.
//!
//! # Algorithm
//! 1. **Mark Phase**: Computes the transitive closure of the `references` relation to find all reachable paths.
//!    Joins this with the `roots` relation to find all objects indirectly reachable from the roots.
//!    Unions these indirect objects with the direct roots to compute the complete `reachable` set.
//! 2. **Sweep Phase**: Performs a relational difference `Heap MINUS Reachable` to identify garbage.

use relvar_core::{error::DatabaseError, values::Relation};

/// A Relational Garbage Collector.
pub struct GarbageCollector {
    /// Root pointers. Schema: `(address: Int)`
    pub roots: Relation,
    /// Allocated objects. Schema: `(address: Int, size: Int)`
    pub heap: Relation,
    /// Pointers between objects. Schema: `(from_addr: Int, to_addr: Int)`
    pub references: Relation,
}

impl GarbageCollector {
    /// Creates a new GarbageCollector.
    pub fn new(roots: Relation, heap: Relation, references: Relation) -> Self {
        Self {
            roots,
            heap,
            references,
        }
    }

    /// Performs the Mark-and-Sweep algorithm.
    /// Returns a relation of garbage objects with schema `(address: Int, size: Int)`.
    pub fn mark_and_sweep(&self) -> Result<Relation, DatabaseError> {
        // 1. Find all paths using transitive closure
        let closure = self.references.tclose("from_addr", "to_addr")?;

        // 2. Find objects reachable from roots
        let roots_as_from = self.roots.rename(&[("address", "from_addr")]);
        let reachable_from_roots = closure.join(&roots_as_from)?;

        // Ensure that to_addr has same schema as roots by projecting and renaming
        let reachable_indirect = reachable_from_roots
            .project(&["to_addr"])
            .rename(&[("to_addr", "address")]);

        // 3. Union with direct roots to get all reachable objects
        let all_reachable = reachable_indirect
            .union(&self.roots)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Find garbage addresses (heap addresses MINUS reachable addresses)
        let all_heap_addresses = self.heap.project(&["address"]);
        let garbage_addresses = all_heap_addresses
            .difference(&all_reachable)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Join with heap to get full garbage object info (address, size)
        let garbage = self.heap.join(&garbage_addresses)?;

        Ok(garbage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_mark_and_sweep() {
        let root_type = TupleType::new().with_attribute("address", ScalarType::Int);
        let mut roots = Relation::new(RelationType::new(root_type));

        let heap_type = TupleType::new()
            .with_attribute("address", ScalarType::Int)
            .with_attribute("size", ScalarType::Int);
        let mut heap = Relation::new(RelationType::new(heap_type));

        let ref_type = TupleType::new()
            .with_attribute("from_addr", ScalarType::Int)
            .with_attribute("to_addr", ScalarType::Int);
        let mut references = Relation::new(RelationType::new(ref_type));

        // Setup memory:
        // Root -> 1
        // 1 -> 2
        // 2 -> 3
        // 4 -> 5 (Unreachable cycle or disconnected component)
        // 6 (Completely isolated)

        roots.insert(tuple! { address: 1i64 }).unwrap();

        heap.insert(tuple! { address: 1i64, size: 10i64 }).unwrap();
        heap.insert(tuple! { address: 2i64, size: 20i64 }).unwrap();
        heap.insert(tuple! { address: 3i64, size: 30i64 }).unwrap();
        heap.insert(tuple! { address: 4i64, size: 40i64 }).unwrap();
        heap.insert(tuple! { address: 5i64, size: 50i64 }).unwrap();
        heap.insert(tuple! { address: 6i64, size: 60i64 }).unwrap();

        references
            .insert(tuple! { from_addr: 1i64, to_addr: 2i64 })
            .unwrap();
        references
            .insert(tuple! { from_addr: 2i64, to_addr: 3i64 })
            .unwrap();
        references
            .insert(tuple! { from_addr: 4i64, to_addr: 5i64 })
            .unwrap();

        let gc = GarbageCollector::new(roots, heap, references);
        let garbage = gc.mark_and_sweep().unwrap();

        assert_eq!(garbage.cardinality(), 3);

        let mut g_addrs: Vec<i64> = garbage
            .tuples()
            .map(|t| t.get_typed::<i64>("address").unwrap())
            .collect();
        g_addrs.sort();

        // 4, 5, 6 should be garbage
        assert_eq!(g_addrs, vec![4, 5, 6]);
    }
}
