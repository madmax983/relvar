//! Relational Garbage Collector
//!
//! Models a Mark-and-Sweep Garbage Collector using pure relational algebra.
//! This demonstrates how complex algorithms like memory management can be expressed
//! purely declaratively using standard relation operations (`tclose`, `join`, `union`, `difference`).

use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A Mark-and-Sweep Garbage Collector modeled purely using relational algebra.
pub struct GarbageCollector {
    /// A relation of all objects in the heap: { addr: Int }
    pub heap: Relation,
    /// A relation of root references: { addr: Int }
    pub roots: Relation,
    /// A relation of references between objects: { from_addr: Int, to_addr: Int }
    pub references: Relation,
}

impl Default for GarbageCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl GarbageCollector {
    /// Bootstraps an empty garbage collector.
    pub fn new() -> Self {
        let addr_type = RelationType::new(TupleType::new().with_attribute("addr", ScalarType::Int));
        let ref_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_addr", ScalarType::Int)
                .with_attribute("to_addr", ScalarType::Int),
        );

        Self {
            heap: Relation::new(addr_type.clone()),
            roots: Relation::new(addr_type),
            references: Relation::new(ref_type),
        }
    }

    /// Adds a root reference address.
    pub fn add_root(&mut self, addr: i64) -> Result<(), DatabaseError> {
        self.roots
            .insert(crate::tuple! { addr: addr })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Adds an allocated object address to the heap.
    pub fn add_object(&mut self, addr: i64) -> Result<(), DatabaseError> {
        self.heap
            .insert(crate::tuple! { addr: addr })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Adds a reference from one object to another.
    pub fn add_reference(&mut self, from_addr: i64, to_addr: i64) -> Result<(), DatabaseError> {
        self.references
            .insert(crate::tuple! { from_addr: from_addr, to_addr: to_addr })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Computes the set of reachable objects (Mark phase) and returns the set of unreachable objects (Sweep phase).
    pub fn sweep(&self) -> Result<Relation, DatabaseError> {
        // Mark Phase
        // Find all transitive references: { from_addr, to_addr }
        let closure = self.references.tclose("from_addr", "to_addr")?;

        // Find roots with their transitive references
        let renamed_roots = self.roots.rename(&[("addr", "from_addr")]);
        let reachable_from_roots = renamed_roots.join(&closure)?;

        // Extract reachable addresses
        let marked_indirect = reachable_from_roots
            .project(&["to_addr"])
            .rename(&[("to_addr", "addr")]);

        // Union with direct roots
        let all_marked = self
            .roots
            .union(&marked_indirect)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Sweep Phase: heap \ all_marked
        self.heap
            .difference(&all_marked)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_garbage_collector() {
        let mut gc = GarbageCollector::new();

        // Heap: 1, 2, 3, 4, 5
        for i in 1..=5 {
            gc.add_object(i).unwrap();
        }

        // Root: 1
        gc.add_root(1).unwrap();

        // References: 1 -> 2, 2 -> 3, 4 -> 5
        gc.add_reference(1, 2).unwrap();
        gc.add_reference(2, 3).unwrap();
        gc.add_reference(4, 5).unwrap();

        // Sweep should find 4 and 5 as garbage
        let garbage = gc.sweep().unwrap();

        assert_eq!(garbage.cardinality(), 2);

        let t4 = crate::tuple! { addr: 4i64 };
        let t5 = crate::tuple! { addr: 5i64 };

        assert!(garbage.contains(&t4));
        assert!(garbage.contains(&t5));
    }
}
