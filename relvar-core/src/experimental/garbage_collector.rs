use crate::{DatabaseError, Relation};

/// A Relational Garbage Collector that implements Mark-and-Sweep.
pub struct GarbageCollector;

impl GarbageCollector {
    /// Identifies garbage by performing a mark-and-sweep algorithm using purely relational algebra.
    ///
    /// * `roots`: Relation with a single attribute `id` (ScalarType::Int) representing root pointers.
    /// * `pointers`: Relation with attributes `from` (Int) and `to` (Int) representing references.
    /// * `heap`: Relation containing at least an `id` (Int) attribute representing allocated objects.
    pub fn sweep(
        roots: &Relation,
        pointers: &Relation,
        heap: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // The Mark phase:
        // 1. Compute the transitive closure of all pointer paths.
        let all_paths = pointers.tclose("from", "to")?;

        // 2. Find all nodes reachable from the roots.
        let reachable = roots
            .rename(&[("id", "from")])
            .join(&all_paths)?
            .project(&["to"])
            .rename(&[("to", "id")]);

        // 3. The completely marked set is the union of roots and reachable nodes.
        let marked = roots
            .union(&reachable)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // The Sweep phase:
        // 4. Garbage is any allocated object not in the marked set.
        let heap_ids = heap.project(&["id"]);
        let garbage = heap_ids
            .difference(&marked)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(garbage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RelationType, ScalarType, TupleType, tuple};

    #[test]
    fn test_mark_and_sweep() -> Result<(), Box<dyn std::error::Error>> {
        let id_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        let ptr_type = RelationType::new(
            TupleType::new()
                .with_attribute("from", ScalarType::Int)
                .with_attribute("to", ScalarType::Int),
        );

        let mut roots = Relation::new(id_type.clone());
        let _ = roots.insert(tuple! { id: 1i64 })?;

        let mut pointers = Relation::new(ptr_type.clone());
        // Root 1 points to 2
        let _ = pointers.insert(tuple! { from: 1i64, to: 2i64 })?;
        // 2 points to 3
        let _ = pointers.insert(tuple! { from: 2i64, to: 3i64 })?;
        // Cycle: 3 points to 2
        let _ = pointers.insert(tuple! { from: 3i64, to: 2i64 })?;

        // Isolated garbage cycle: 4 points to 5, 5 points to 4
        let _ = pointers.insert(tuple! { from: 4i64, to: 5i64 })?;
        let _ = pointers.insert(tuple! { from: 5i64, to: 4i64 })?;

        // 6 is isolated garbage

        let mut heap = Relation::new(id_type.clone());
        let _ = heap.insert(tuple! { id: 1i64 })?;
        let _ = heap.insert(tuple! { id: 2i64 })?;
        let _ = heap.insert(tuple! { id: 3i64 })?;
        let _ = heap.insert(tuple! { id: 4i64 })?;
        let _ = heap.insert(tuple! { id: 5i64 })?;
        let _ = heap.insert(tuple! { id: 6i64 })?;

        let garbage = GarbageCollector::sweep(&roots, &pointers, &heap)?;

        assert_eq!(garbage.cardinality(), 3);

        let mut garbage_ids: Vec<i64> = garbage
            .tuples()
            .map(|t| t.get_typed::<i64>("id").unwrap())
            .collect();
        garbage_ids.sort();

        assert_eq!(garbage_ids, vec![4, 5, 6]);

        Ok(())
    }
}
