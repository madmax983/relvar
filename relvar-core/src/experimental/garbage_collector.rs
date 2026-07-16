//! Relational Mark-and-Sweep Garbage Collector.
//!
//! This module implements a Mark-and-Sweep garbage collector using pure relational algebra.
//! Memory is represented as relations: `roots`, `heap`, and `references`.
//! The "Mark" phase is computed via transitive closure over references from the roots.
//! The "Sweep" phase identifies unreferenced objects via relational difference.

use crate::error::DatabaseError;

use crate::values::Relation;

/// Performs the Mark-and-Sweep algorithm using purely relational algebra.
///
/// `roots` is a relation of starting object IDs. It must have an attribute `obj_id` (Int).
/// `heap` is a relation of all allocated objects. It must have an attribute `obj_id` (Int).
/// `references` is a relation mapping from one object to another. It must have attributes `from_id` (Int) and `to_id` (Int).
///
/// Returns a `Relation` containing the unreferenced (garbage) objects to be swept.
/// This relation will have the same schema as `heap`.
///
/// # Examples
/// ```
/// use relvar_core::tuple;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar_core::experimental::garbage_collector::mark_and_sweep;
///
/// let id_heading = TupleType::new().with_attribute("obj_id", ScalarType::Int);
/// let ref_heading = TupleType::new()
///     .with_attribute("from_id", ScalarType::Int)
///     .with_attribute("to_id", ScalarType::Int);
///
/// let mut roots = Relation::new(RelationType::new(id_heading.clone()));
/// roots.insert(tuple! { obj_id: 1i64 }).unwrap();
///
/// let mut heap = Relation::new(RelationType::new(id_heading));
/// heap.insert(tuple! { obj_id: 1i64 }).unwrap();
/// heap.insert(tuple! { obj_id: 2i64 }).unwrap();
/// heap.insert(tuple! { obj_id: 3i64 }).unwrap();
/// heap.insert(tuple! { obj_id: 4i64 }).unwrap();
///
/// let mut references = Relation::new(RelationType::new(ref_heading));
/// references.insert(tuple! { from_id: 1i64, to_id: 2i64 }).unwrap();
/// references.insert(tuple! { from_id: 3i64, to_id: 4i64 }).unwrap();
///
/// // Object 1 is root. It references 2. So 1 and 2 are reachable.
/// // Object 3 and 4 are not reachable from any root.
/// let garbage = mark_and_sweep(&roots, &heap, &references).unwrap();
///
/// assert_eq!(garbage.cardinality(), 2);
/// assert!(garbage.contains(&tuple! { obj_id: 3i64 }));
/// assert!(garbage.contains(&tuple! { obj_id: 4i64 }));
/// ```
pub fn mark_and_sweep(
    roots: &Relation,
    heap: &Relation,
    references: &Relation,
) -> Result<Relation, DatabaseError> {
    // Phase 1: Mark (find reachable objects)

    // First, find all paths in the reference graph via transitive closure
    // Result has: (from_id, to_id)
    let all_paths = references.tclose("from_id", "to_id")?;

    // Rename roots obj_id to from_id to match the paths relation
    let roots_renamed = roots.rename(&[("obj_id", "from_id")]);

    // Join roots with paths to find all reachable downstream nodes from roots
    // Result has: (from_id, to_id)
    let reachable_from_roots = roots_renamed.join(&all_paths)?;

    // Project to just the reachable nodes (to_id) and rename to obj_id
    let indirectly_reachable = reachable_from_roots
        .project(&["to_id"])
        .rename(&[("to_id", "obj_id")]);

    // Also consider objects immediately referenced by roots
    let immediate_refs = roots_renamed.join(references)?;
    let immediate_reachable = immediate_refs
        .project(&["to_id"])
        .rename(&[("to_id", "obj_id")]);

    // The total reachable set is the roots themselves UNION the indirectly reachable ones UNION immediate ones
    let reachable1 = roots
        .union(&indirectly_reachable)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
    let all_reachable = reachable1
        .union(&immediate_reachable)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Phase 2: Sweep (identify garbage)
    // Garbage = all objects in heap MINUS reachable objects
    heap.difference(&all_reachable)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::tuple;

    #[test]
    fn test_mark_and_sweep() {
        let id_heading = TupleType::new().with_attribute("obj_id", ScalarType::Int);
        let ref_heading = TupleType::new()
            .with_attribute("from_id", ScalarType::Int)
            .with_attribute("to_id", ScalarType::Int);

        let mut roots = Relation::new(RelationType::new(id_heading.clone()));
        roots.insert(tuple! { obj_id: 1i64 }).unwrap();

        let mut heap = Relation::new(RelationType::new(id_heading));
        heap.insert(tuple! { obj_id: 1i64 }).unwrap();
        heap.insert(tuple! { obj_id: 2i64 }).unwrap();
        heap.insert(tuple! { obj_id: 3i64 }).unwrap();
        heap.insert(tuple! { obj_id: 4i64 }).unwrap();
        heap.insert(tuple! { obj_id: 5i64 }).unwrap();

        let mut references = Relation::new(RelationType::new(ref_heading));
        // Path: 1 -> 2 -> 3
        references
            .insert(tuple! { from_id: 1i64, to_id: 2i64 })
            .unwrap();
        references
            .insert(tuple! { from_id: 2i64, to_id: 3i64 })
            .unwrap();

        // Cycle: 4 -> 5 -> 4 (but disconnected from roots)
        references
            .insert(tuple! { from_id: 4i64, to_id: 5i64 })
            .unwrap();
        references
            .insert(tuple! { from_id: 5i64, to_id: 4i64 })
            .unwrap();

        let garbage = mark_and_sweep(&roots, &heap, &references).unwrap();

        assert_eq!(garbage.cardinality(), 2);
        assert!(garbage.contains(&tuple! { obj_id: 4i64 }));
        assert!(garbage.contains(&tuple! { obj_id: 5i64 }));
    }
}
