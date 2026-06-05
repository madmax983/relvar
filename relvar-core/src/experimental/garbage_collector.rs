//! Relational Mark-and-Sweep Garbage Collector.
//!
//! This module implements a Mark-and-Sweep Garbage Collector using pure
//! relational algebra. The heap, roots, and references are represented
//! as relations.

use crate::error::DatabaseError;
#[cfg(test)]
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// Computes the set of unreachable (garbage) objects in the heap.
///
/// # Arguments
/// * `roots` - A relation with a single attribute `id` (Int) representing root objects.
/// * `heap` - A relation with a single attribute `id` (Int) representing all allocated objects.
/// * `references` - A relation with `from_id` (Int) and `to_id` (Int) representing pointers.
///
/// # Returns
/// A relation with a single attribute `id` (Int) representing objects that cannot be
/// reached from any root.
///
/// # Examples
///
/// ```
/// use relvar_core::experimental::garbage_collector::find_garbage;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar_core::tuple;
///
/// let id_heading = TupleType::new().with_attribute("id", ScalarType::Int);
/// let ref_heading = TupleType::new()
///     .with_attribute("from_id", ScalarType::Int)
///     .with_attribute("to_id", ScalarType::Int);
///
/// let mut roots = Relation::new(RelationType::new(id_heading.clone()));
/// roots.insert(tuple! { id: 1i64 }).unwrap();
///
/// let mut heap = Relation::new(RelationType::new(id_heading.clone()));
/// heap.insert(tuple! { id: 1i64 }).unwrap();
/// heap.insert(tuple! { id: 2i64 }).unwrap();
/// heap.insert(tuple! { id: 3i64 }).unwrap();
///
/// let mut references = Relation::new(RelationType::new(ref_heading));
/// references.insert(tuple! { from_id: 1i64, to_id: 2i64 }).unwrap();
///
/// // 1 is root, 2 is referenced by 1. 3 is garbage.
/// let garbage = find_garbage(&roots, &heap, &references).unwrap();
///
/// assert_eq!(garbage.cardinality(), 1);
/// assert!(garbage.contains(&tuple! { id: 3i64 }));
/// ```
pub fn find_garbage(
    roots: &Relation,
    heap: &Relation,
    references: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Mark Phase: Find all reachable objects.

    // Find all transitive references: (from_id, to_id)
    let closure = references.tclose("from_id", "to_id")?;

    // Join closure with roots to find objects reachable from roots
    let renamed_roots = roots.rename(&[("id", "from_id")]);
    let reachable_from_roots = closure.join(&renamed_roots)?;

    // The reachable objects are the `to_id`s. We project and rename to `id`.
    let reachable_indirect = reachable_from_roots
        .project(&["to_id"])
        .rename(&[("to_id", "id")]);

    // Also, roots themselves are reachable.
    let all_reachable = reachable_indirect.union(roots).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 2. Sweep Phase: Find garbage by taking the difference between heap and reachable.
    let garbage = heap.difference(&all_reachable).map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    Ok(garbage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;

    #[test]
    fn test_mark_and_sweep() {
        let id_heading = TupleType::new().with_attribute("id", ScalarType::Int);
        let ref_heading = TupleType::new()
            .with_attribute("from_id", ScalarType::Int)
            .with_attribute("to_id", ScalarType::Int);

        let mut roots = Relation::new(RelationType::new(id_heading.clone()));
        roots.insert(tuple! { id: 1i64 }).unwrap();

        let mut heap = Relation::new(RelationType::new(id_heading.clone()));
        for i in 1..=6 {
            heap.insert(tuple! { id: i as i64 }).unwrap();
        }

        let mut references = Relation::new(RelationType::new(ref_heading));
        // 1 -> 2
        references
            .insert(tuple! { from_id: 1i64, to_id: 2i64 })
            .unwrap();
        // 2 -> 3
        references
            .insert(tuple! { from_id: 2i64, to_id: 3i64 })
            .unwrap();
        // 4 -> 5 (Unreachable cycle or chain)
        references
            .insert(tuple! { from_id: 4i64, to_id: 5i64 })
            .unwrap();
        // 5 -> 4
        references
            .insert(tuple! { from_id: 5i64, to_id: 4i64 })
            .unwrap();

        // Reachable: 1 (root), 2, 3
        // Garbage: 4, 5, 6

        let garbage = find_garbage(&roots, &heap, &references).unwrap();

        assert_eq!(garbage.cardinality(), 3);
        assert!(garbage.contains(&tuple! { id: 4i64 }));
        assert!(garbage.contains(&tuple! { id: 5i64 }));
        assert!(garbage.contains(&tuple! { id: 6i64 }));
    }
}
