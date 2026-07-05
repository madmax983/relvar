use crate::algebra::{Aggregation, AggregationFn};
use crate::error::DatabaseError;
use crate::types::ScalarType;
use crate::values::{Relation, ScalarValue};

/// Relational Mark-and-Sweep Garbage Collector
///
/// `heap` relation must have `id` (Int) and `size` (Int).
/// `roots` relation must have `id` (Int).
/// `references` relation must have `from_id` (Int) and `to_id` (Int).
pub fn mark_and_sweep(
    heap: &Relation,
    roots: &Relation,
    references: &Relation,
) -> Result<(Relation, i64), DatabaseError> {
    // 1. Compute reachable references using transitive closure
    let all_paths = references.tclose("from_id", "to_id")?;

    // 2. Find nodes reachable from roots
    // Rename roots `id` to `from_id` to join with all_paths
    let roots_as_from = roots.rename(&[("id", "from_id")]);
    let reachable_from_roots = roots_as_from.join(&all_paths)?;

    // 3. Project to just the reachable `to_id` and rename to `id`
    let reachable_ids = reachable_from_roots
        .project(&["to_id"])
        .rename(&[("to_id", "id")]);

    // 4. Union with the roots themselves, as roots are also reachable
    let all_reachable = reachable_ids
        .union(roots)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 5. Sweep: Find garbage by taking the difference between all heap ids and all_reachable
    let all_heap_ids = heap.project(&["id"]);
    let garbage_ids = all_heap_ids
        .difference(&all_reachable)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 6. Join garbage_ids with heap to get the sizes
    let garbage = heap.join(&garbage_ids)?;

    // 7. Summarize to find total reclaimed memory
    let total_reclaimed_rel = garbage
        .summarize(
            &[],
            &[Aggregation {
                result_name: "total_size".to_string(),
                result_type: ScalarType::Int,
                function: AggregationFn::Sum("size".to_string()),
            }],
        )
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let mut total_size = 0;
    if total_reclaimed_rel.cardinality() > 0 {
        let tuple = total_reclaimed_rel.tuples().next().unwrap();
        if let Some(ScalarValue::Int(size)) = tuple.get("total_size") {
            total_size = *size;
        }
    }

    Ok((garbage, total_size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, TupleType};

    fn make_heap() -> Relation {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("size", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading));
        rel.insert(tuple! {id: 1i64, size: 10i64}).unwrap();
        rel.insert(tuple! {id: 2i64, size: 20i64}).unwrap();
        rel.insert(tuple! {id: 3i64, size: 30i64}).unwrap();
        rel.insert(tuple! {id: 4i64, size: 40i64}).unwrap();
        rel.insert(tuple! {id: 5i64, size: 50i64}).unwrap();
        rel
    }

    fn make_roots() -> Relation {
        let heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading));
        rel.insert(tuple! {id: 1i64}).unwrap();
        rel
    }

    fn make_references() -> Relation {
        let heading = TupleType::new()
            .with_attribute("from_id", ScalarType::Int)
            .with_attribute("to_id", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading));
        rel.insert(tuple! {from_id: 1i64, to_id: 2i64}).unwrap(); // Root -> 2
        rel.insert(tuple! {from_id: 2i64, to_id: 3i64}).unwrap(); // 2 -> 3 (Reachable)
        rel.insert(tuple! {from_id: 4i64, to_id: 5i64}).unwrap(); // 4 -> 5 (Garbage)
        rel
    }

    #[test]
    fn test_mark_and_sweep() {
        let heap = make_heap();
        let roots = make_roots();
        let references = make_references();

        let (garbage, total_reclaimed) = mark_and_sweep(&heap, &roots, &references).unwrap();

        // Nodes 4 and 5 should be garbage. Total size = 40 + 50 = 90
        assert_eq!(garbage.cardinality(), 2);
        assert_eq!(total_reclaimed, 90);
    }
}
