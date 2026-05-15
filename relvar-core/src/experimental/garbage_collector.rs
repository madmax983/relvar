//! Relational Garbage Collector
//!
//! Models a Mark-and-Sweep Garbage Collector using purely relational algebra.
//! The Mark phase computes reachability via transitive closure (`tclose`), `join`,
//! and `union`. The Sweep phase identifies garbage via `difference`.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Computes unreachable objects (garbage) using the Mark-and-Sweep algorithm
/// modeled purely with relational algebra.
///
/// # Arguments
///
/// * `roots` - A relation with a single attribute `ptr` representing root pointers.
/// * `heap` - A relation with a single attribute `ptr` representing all allocated objects.
/// * `references` - A relation with attributes `from` and `to` representing edges.
///
/// # Returns
///
/// A relation with a single attribute `ptr` containing all unreachable objects.
pub fn find_garbage(
    roots: &Relation,
    heap: &Relation,
    references: &Relation,
) -> Result<Relation, DatabaseError> {
    // Mark Phase: Find all objects reachable from roots

    // 1. Compute transitive closure of references to find all paths
    let paths = references.tclose("from", "to")?;

    // 2. Join roots with paths.
    // roots has `ptr`. We rename `ptr` -> `from` to join with paths(from, to).
    let renamed_roots = roots.rename(&[("ptr", "from")]);
    let reachable_from_roots = renamed_roots.join(&paths)?;

    // 3. Extract the reachable target pointers and rename back to `ptr`
    let indirectly_reachable = reachable_from_roots
        .project(&["to"])
        .rename(&[("to", "ptr")]);

    // 4. All reachable objects are roots UNION indirectly reachable
    let all_reachable = roots
        .clone()
        .union_into(&indirectly_reachable)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Sweep Phase: Garbage is all heap objects MINUS reachable objects
    let garbage = heap
        .clone()
        .difference_into(&all_reachable)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    Ok(garbage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    fn create_ptr_relation(ptrs: &[i64]) -> Relation {
        let heading = TupleType::new().with_attribute("ptr", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        for &p in ptrs {
            r.insert(tuple! { ptr: p }).unwrap();
        }
        r
    }

    fn create_refs_relation(edges: &[(i64, i64)]) -> Relation {
        let heading = TupleType::new()
            .with_attribute("from", ScalarType::Int)
            .with_attribute("to", ScalarType::Int);
        let mut r = Relation::new(RelationType::new(heading));
        for &(f, t) in edges {
            r.insert(tuple! { from: f, to: t }).unwrap();
        }
        r
    }

    #[test]
    fn test_garbage_collector_acyclic() {
        // Roots: 1
        // Heap: 1, 2, 3, 4
        // Refs: 1 -> 2, 2 -> 3
        // Garbage should be: 4
        let roots = create_ptr_relation(&[1]);
        let heap = create_ptr_relation(&[1, 2, 3, 4]);
        let refs = create_refs_relation(&[(1, 2), (2, 3)]);

        let garbage = find_garbage(&roots, &heap, &refs).unwrap();

        assert_eq!(garbage.cardinality(), 1);
        assert!(garbage.contains(&tuple! { ptr: 4i64 }));
    }

    #[test]
    fn test_garbage_collector_cyclic() {
        // Roots: 1
        // Heap: 1, 2, 3, 4, 5
        // Refs: 1 -> 2, 2 -> 3, 3 -> 2 (cycle)
        //       4 -> 5, 5 -> 4 (disconnected cycle)
        // Garbage should be: 4, 5
        let roots = create_ptr_relation(&[1]);
        let heap = create_ptr_relation(&[1, 2, 3, 4, 5]);
        let refs = create_refs_relation(&[(1, 2), (2, 3), (3, 2), (4, 5), (5, 4)]);

        let garbage = find_garbage(&roots, &heap, &refs).unwrap();

        assert_eq!(garbage.cardinality(), 2);
        assert!(garbage.contains(&tuple! { ptr: 4i64 }));
        assert!(garbage.contains(&tuple! { ptr: 5i64 }));
    }

    #[test]
    fn test_garbage_collector_disconnected() {
        // Roots: 1
        // Heap: 1, 2, 3
        // Refs: <empty>
        // Garbage should be: 2, 3
        let roots = create_ptr_relation(&[1]);
        let heap = create_ptr_relation(&[1, 2, 3]);
        let refs = create_refs_relation(&[]);

        let garbage = find_garbage(&roots, &heap, &refs).unwrap();

        assert_eq!(garbage.cardinality(), 2);
        assert!(garbage.contains(&tuple! { ptr: 2i64 }));
        assert!(garbage.contains(&tuple! { ptr: 3i64 }));
    }
}
