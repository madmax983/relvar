use crate::error::DatabaseError;
use crate::values::Relation;

/// Evaluates a Mark-and-Sweep Garbage Collector purely using relational algebra.
///
/// The roots and references are modeled as relations. Reachability is computed using
/// transitive closure (`tclose`), and garbage is identified via set difference.
pub fn mark_and_sweep(
    roots: &Relation,
    heap: &Relation,
    references: &Relation,
) -> Result<(Relation, Relation), DatabaseError> {
    // 1. Mark Phase: Find all reachable objects.
    // Transitive closure of references
    let paths = references.tclose("from_id", "to_id")?;

    // Rename from_id to id in paths so we can join with roots
    let renamed_paths = paths.rename(&[("from_id", "id")]);

    // Join roots with paths to find all descendants of roots
    let reachable_descendants = roots
        .join(&renamed_paths)?
        .project(&["to_id"])
        .rename_into(&[("to_id", "id")]);

    // The set of all live objects is the roots unioned with their descendants
    let live_objects = roots
        .project(&["id"])
        .union(&reachable_descendants)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 2. Sweep Phase: Identify garbage and surviving heap objects.
    let heap_ids = heap.project(&["id"]);
    let garbage_ids = heap_ids
        .difference(&live_objects)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let new_heap = heap.join(&live_objects)?;

    Ok((new_heap, garbage_ids))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_mark_and_sweep() {
        let heading_id = TupleType::new().with_attribute("id", ScalarType::Int);

        let heading_heap = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("data", ScalarType::String);

        let heading_refs = TupleType::new()
            .with_attribute("from_id", ScalarType::Int)
            .with_attribute("to_id", ScalarType::Int);

        let mut roots = Relation::new(RelationType::new(heading_id.clone()));
        roots.insert(tuple! { id: 1i64 }).unwrap();

        let mut heap = Relation::new(RelationType::new(heading_heap.clone()));
        heap.insert(tuple! { id: 1i64, data: "RootObj" }).unwrap();
        heap.insert(tuple! { id: 2i64, data: "Reachable1" })
            .unwrap();
        heap.insert(tuple! { id: 3i64, data: "Reachable2" })
            .unwrap();
        heap.insert(tuple! { id: 4i64, data: "Garbage1" }).unwrap();
        heap.insert(tuple! { id: 5i64, data: "Garbage2" }).unwrap();

        let mut references = Relation::new(RelationType::new(heading_refs.clone()));
        references
            .insert(tuple! { from_id: 1i64, to_id: 2i64 })
            .unwrap();
        references
            .insert(tuple! { from_id: 2i64, to_id: 3i64 })
            .unwrap();
        // Disconnected garbage cycle
        references
            .insert(tuple! { from_id: 4i64, to_id: 5i64 })
            .unwrap();
        references
            .insert(tuple! { from_id: 5i64, to_id: 4i64 })
            .unwrap();

        let (new_heap, garbage_ids) = mark_and_sweep(&roots, &heap, &references).unwrap();

        // Assert reachable items exist
        assert_eq!(new_heap.cardinality(), 3);

        // Assert garbage items found
        assert_eq!(garbage_ids.cardinality(), 2);
    }
}
