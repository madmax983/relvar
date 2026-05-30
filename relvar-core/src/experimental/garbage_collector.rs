use crate::tuple;
use crate::types::RelationType;
use crate::{DatabaseError, Relation, ScalarType, TupleType};

/// A relational Mark-and-Sweep Garbage Collector.
/// Represents `roots`, `heap`, and `references` as relations.
pub struct GarbageCollector {
    /// The set of all allocated nodes (objects).
    pub heap: Relation,
    /// The set of root nodes.
    pub roots: Relation,
    /// The set of references between nodes.
    pub references: Relation,
}

impl GarbageCollector {
    /// Creates a new, empty Garbage Collector.
    pub fn new() -> Self {
        let node_type =
            RelationType::new(TupleType::new().with_attribute("node_id", ScalarType::Int));
        let ref_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_id", ScalarType::Int)
                .with_attribute("to_id", ScalarType::Int),
        );

        GarbageCollector {
            heap: Relation::new(node_type.clone()),
            roots: Relation::new(node_type),
            references: Relation::new(ref_type),
        }
    }

    /// Allocates a new node in the heap.
    pub fn allocate(&mut self, node_id: i64) -> Result<(), DatabaseError> {
        self.heap.insert(tuple! { node_id: node_id })?;
        Ok(())
    }

    /// Adds a node to the set of roots.
    pub fn add_root(&mut self, node_id: i64) -> Result<(), DatabaseError> {
        self.roots.insert(tuple! { node_id: node_id })?;
        Ok(())
    }

    /// Adds a directed reference from one node to another.
    pub fn add_reference(&mut self, from_id: i64, to_id: i64) -> Result<(), DatabaseError> {
        self.references
            .insert(tuple! { from_id: from_id, to_id: to_id })?;
        Ok(())
    }

    /// Performs the mark-and-sweep algorithm and returns the set of garbage nodes.
    pub fn mark_and_sweep(&self) -> Result<Relation, DatabaseError> {
        // Step 1: Find all paths via transitive closure
        let paths = self.references.tclose("from_id", "to_id")?;

        // Step 2: Join roots with paths to find all reachable descendants
        // Rename roots' node_id to from_id to match paths' from_id
        let renamed_roots = self.roots.clone().rename(&[("node_id", "from_id")]);

        // Join to find all paths starting from a root
        let reachable_from_roots = paths.join(&renamed_roots)?;

        // Project the destination nodes and rename to node_id
        let reached_nodes = reachable_from_roots
            .project(&["to_id"])
            .rename(&[("to_id", "node_id")]);

        // Step 3: Union original roots with reached nodes
        let all_reachable = self
            .roots
            .clone()
            .union(&reached_nodes)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Step 4: Sweep - garbage is the difference between heap and all_reachable
        let garbage = self
            .heap
            .difference(&all_reachable)
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

        // Allocate nodes 1 through 6
        for i in 1..=6 {
            gc.allocate(i).unwrap();
        }

        // Roots: 1, 2
        gc.add_root(1).unwrap();
        gc.add_root(2).unwrap();

        // References:
        // 1 -> 3
        // 3 -> 4
        // 5 -> 6 (unreachable from roots)
        gc.add_reference(1, 3).unwrap();
        gc.add_reference(3, 4).unwrap();
        gc.add_reference(5, 6).unwrap();

        // Perform GC
        let garbage = gc.mark_and_sweep().unwrap();

        // Reachable: 1, 2, 3, 4
        // Garbage: 5, 6
        assert_eq!(garbage.cardinality(), 2);

        let garbage_ids: std::collections::HashSet<i64> = garbage
            .tuples()
            .map(|t| t.get_typed::<i64>("node_id").unwrap())
            .collect();

        assert!(garbage_ids.contains(&5));
        assert!(garbage_ids.contains(&6));
    }
}
