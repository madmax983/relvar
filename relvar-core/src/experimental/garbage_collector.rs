use crate::error::DatabaseError;
use crate::values::Relation;

/// A Mark-and-Sweep Garbage Collector implemented purely with Relational Algebra!
pub struct RelationalGC;

impl RelationalGC {
    /// Runs the mark-and-sweep algorithm.
    ///
    /// `roots`: Relation with `node_id` (Int)
    /// `heap`: Relation with `node_id` (Int)
    /// `references`: Relation with `from_id` (Int), `to_id` (Int)
    ///
    /// Returns a relation containing the `node_id`s of the garbage (unreachable nodes).
    pub fn collect_garbage(
        roots: &Relation,
        heap: &Relation,
        references: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // 1. Mark phase: Find all nodes reachable from roots.

        // First, compute the transitive closure of all references.
        // This gives us all paths: (from_id, to_id)
        let paths = references.tclose("from_id", "to_id")?;

        // Find reachable nodes by joining roots with paths.
        // We need to match roots.node_id with paths.from_id
        // So we rename roots.node_id to from_id.
        let renamed_roots = roots.rename(&[("node_id", "from_id")]);

        // Join roots with paths. This gives (from_id, to_id) where from_id is a root.
        let reachable_edges = renamed_roots
            .join(&paths)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Project to get just the reached nodes (to_id), and rename to node_id.
        let reached_from_roots = reachable_edges
            .project(&["to_id"])
            .rename(&[("to_id", "node_id")]);

        // The total reachable set is the roots themselves UNION the nodes reached from roots.
        let all_reachable = roots
            .union(&reached_from_roots)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Sweep phase: Garbage is everything in the heap that is not reachable.
        let garbage = heap
            .difference(&all_reachable)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(garbage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_mark_and_sweep() {
        let node_type =
            RelationType::new(TupleType::new().with_attribute("node_id", ScalarType::Int));
        let ref_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_id", ScalarType::Int)
                .with_attribute("to_id", ScalarType::Int),
        );

        let mut roots = Relation::new(node_type.clone());
        roots.insert(tuple! { node_id: 1i64 }).unwrap(); // Root is node 1

        let mut heap = Relation::new(node_type.clone());
        heap.insert(tuple! { node_id: 1i64 }).unwrap();
        heap.insert(tuple! { node_id: 2i64 }).unwrap();
        heap.insert(tuple! { node_id: 3i64 }).unwrap();
        heap.insert(tuple! { node_id: 4i64 }).unwrap(); // 4 will be garbage
        heap.insert(tuple! { node_id: 5i64 }).unwrap(); // 5 will be garbage

        let mut references = Relation::new(ref_type);
        // 1 -> 2
        references
            .insert(tuple! { from_id: 1i64, to_id: 2i64 })
            .unwrap();
        // 2 -> 3
        references
            .insert(tuple! { from_id: 2i64, to_id: 3i64 })
            .unwrap();
        // 4 -> 5 (island of garbage)
        references
            .insert(tuple! { from_id: 4i64, to_id: 5i64 })
            .unwrap();

        let garbage = RelationalGC::collect_garbage(&roots, &heap, &references).unwrap();

        assert_eq!(garbage.cardinality(), 2);

        let mut expected = Relation::new(node_type);
        expected.insert(tuple! { node_id: 4i64 }).unwrap();
        expected.insert(tuple! { node_id: 5i64 }).unwrap();

        assert_eq!(garbage, expected);
    }
}
