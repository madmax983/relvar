use crate::{DatabaseError, Relation};

/// A Relational Mark-and-Sweep Garbage Collector.
///
/// This demonstrates how a memory management algorithm can be modeled entirely
/// using relational algebra (`tclose`, `join`, `union`, `difference`).
pub struct RelationalGC;

impl RelationalGC {
    /// Computes the set of reachable nodes (the "Mark" phase) and subtracts it
    /// from the total heap to find garbage (the "Sweep" phase).
    ///
    /// - `roots`: Relation with a single attribute `id` (ScalarType::Int)
    /// - `heap`: Relation with a single attribute `id` (ScalarType::Int)
    /// - `references`: Relation with attributes `from_id` and `to_id` (ScalarType::Int)
    ///
    /// Returns a relation of garbage nodes with attribute `id`.
    pub fn mark_and_sweep(
        roots: &Relation,
        heap: &Relation,
        references: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // Compute transitive closure of all references to find all paths
        let closure = references.tclose("from_id", "to_id").map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Find which references are reachable from the roots
        let roots_as_from = roots.clone().rename(&[("id", "from_id")]);
        let reachable_edges = roots_as_from.join(&closure).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Extract the target IDs that were reached
        let reached_to_ids = reachable_edges.project(&["to_id"]);
        let reached_ids = reached_to_ids.rename(&[("to_id", "id")]);

        // The marked set is the roots plus anything reached from them
        let marked = roots.union(&reached_ids).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Garbage is anything in the heap that is not marked
        let garbage = heap.difference(&marked).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(garbage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage_engine::InMemoryEngine;
    use crate::{Database, RelationType, ScalarType, TupleType, tuple};

    #[test]
    fn test_mark_and_sweep() {
        let mut db = Database::new(InMemoryEngine::new());

        let root_type = RelationType::new(
            TupleType::new().with_attribute("id", ScalarType::Int)
        );
        let heap_type = RelationType::new(
            TupleType::new().with_attribute("id", ScalarType::Int)
        );
        let ref_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_id", ScalarType::Int)
                .with_attribute("to_id", ScalarType::Int)
        );

        db.create_relvar("roots", root_type).unwrap();
        db.create_relvar("heap", heap_type.clone()).unwrap();
        db.create_relvar("references", ref_type).unwrap();

        // Heap objects 1, 2, 3, 4, 5
        for id in 1..=5 {
            db.insert("heap", tuple! { id: id as i64 }).unwrap();
        }

        // Object 1 is a root
        db.insert("roots", tuple! { id: 1i64 }).unwrap();

        // 1 -> 2
        // 2 -> 3
        // 4 -> 5
        db.insert("references", tuple! { from_id: 1i64, to_id: 2i64 }).unwrap();
        db.insert("references", tuple! { from_id: 2i64, to_id: 3i64 }).unwrap();
        db.insert("references", tuple! { from_id: 4i64, to_id: 5i64 }).unwrap();

        let roots = db.query("roots").unwrap();
        let heap = db.query("heap").unwrap();
        let references = db.query("references").unwrap();

        let garbage = RelationalGC::mark_and_sweep(&roots, &heap, &references).unwrap();

        assert_eq!(garbage.cardinality(), 2); // 4 and 5

        let mut expected = Relation::new(heap_type);
        expected.insert(tuple! { id: 4i64 }).unwrap();
        expected.insert(tuple! { id: 5i64 }).unwrap();

        assert_eq!(garbage, expected);
    }
}
