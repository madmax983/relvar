//! Relational Garbage Collector
//!
//! This module implements a Mark-and-Sweep Garbage Collector purely using relational algebra.
//! It leverages transitive closure (`tclose`) to compute reachable memory sets and
//! set `difference` to identify unreferenced memory.

use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// Performs a Mark-and-Sweep Garbage Collection purely using relational algebra.
///
/// * `roots`: A unary relation with one attribute `node_id` representing root nodes.
/// * `heap`: A unary relation with one attribute `node_id` representing all allocated nodes.
/// * `references`: A binary relation with `from_id` and `to_id` representing directed edges.
///
/// Returns a `Relation` containing the `node_id`s of garbage nodes (nodes in `heap` not reachable from `roots`).
///
/// # Examples
///
/// ```
/// use relvar_core::experimental::garbage_collector::mark_and_sweep;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::{Relation, Tuple};
/// use relvar_core::tuple;
///
/// // Create headings
/// let node_heading = TupleType::new().with_attribute("node_id", ScalarType::Int);
/// let ref_heading = TupleType::new()
///     .with_attribute("from_id", ScalarType::Int)
///     .with_attribute("to_id", ScalarType::Int);
///
/// let mut roots = Relation::new(RelationType::new(node_heading.clone()));
/// roots.insert(tuple!{node_id: 1i64}).unwrap(); // Node 1 is a root
///
/// let mut heap = Relation::new(RelationType::new(node_heading));
/// heap.insert(tuple!{node_id: 1i64}).unwrap();
/// heap.insert(tuple!{node_id: 2i64}).unwrap(); // Reachable from 1
/// heap.insert(tuple!{node_id: 3i64}).unwrap(); // Garbage!
/// heap.insert(tuple!{node_id: 4i64}).unwrap(); // Garbage (circular with 3)
///
/// let mut references = Relation::new(RelationType::new(ref_heading));
/// references.insert(tuple!{from_id: 1i64, to_id: 2i64}).unwrap();
/// references.insert(tuple!{from_id: 3i64, to_id: 4i64}).unwrap();
/// references.insert(tuple!{from_id: 4i64, to_id: 3i64}).unwrap();
///
/// let garbage = mark_and_sweep(&roots, &heap, &references).unwrap();
///
/// // Nodes 3 and 4 should be garbage
/// assert_eq!(garbage.cardinality(), 2);
/// ```
pub fn mark_and_sweep(
    roots: &Relation,
    heap: &Relation,
    references: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Compute reachability via transitive closure
    // If references is empty, tclose returns an empty relation.
    // However, tclose requires the relation to be non-empty and well-formed.
    // We can handle empty references gracefully.
    let closure = if references.is_empty() {
        let heading = TupleType::new()
            .with_attribute("from_id", ScalarType::Int)
            .with_attribute("to_id", ScalarType::Int);
        Relation::new(RelationType::new(heading))
    } else {
        references.tclose("from_id", "to_id")?
    };

    // 2. Find nodes reachable from roots
    let roots_renamed = roots.rename(&[("node_id", "from_id")]);
    let reachable_from_roots = roots_renamed.join(&closure)?;

    let reachable_descendants = reachable_from_roots
        .project(&["to_id"])
        .rename(&[("to_id", "node_id")]);

    // The total reachable set is roots UNION reachable_descendants
    let reachable = roots
        .union(&reachable_descendants)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 3. Sweep: Garbage is everything in the heap that is NOT in the reachable set
    let garbage = heap
        .difference(&reachable)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    Ok(garbage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;

    fn make_node_relation(nodes: &[i64]) -> Relation {
        let heading = TupleType::new().with_attribute("node_id", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading));
        for &node in nodes {
            rel.insert(tuple! {node_id: node}).unwrap();
        }
        rel
    }

    fn make_ref_relation(edges: &[(i64, i64)]) -> Relation {
        let heading = TupleType::new()
            .with_attribute("from_id", ScalarType::Int)
            .with_attribute("to_id", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading));
        for &(from, to) in edges {
            rel.insert(tuple! {from_id: from, to_id: to}).unwrap();
        }
        rel
    }

    #[test]
    fn test_mark_and_sweep_basic() {
        let roots = make_node_relation(&[1]);
        let heap = make_node_relation(&[1, 2, 3, 4, 5]);

        // 1 -> 2 -> 3
        // 4 -> 5 (Unreachable component)
        let references = make_ref_relation(&[(1, 2), (2, 3), (4, 5)]);

        let garbage = mark_and_sweep(&roots, &heap, &references).unwrap();

        assert_eq!(garbage.cardinality(), 2);

        let expected_garbage = make_node_relation(&[4, 5]);
        assert_eq!(
            garbage.difference(&expected_garbage).unwrap().cardinality(),
            0
        );
    }

    #[test]
    fn test_mark_and_sweep_circular() {
        let roots = make_node_relation(&[1]);
        let heap = make_node_relation(&[1, 2, 3, 4]);

        // 1 -> 2
        // 3 -> 4, 4 -> 3 (Unreachable circular component)
        let references = make_ref_relation(&[(1, 2), (3, 4), (4, 3)]);

        let garbage = mark_and_sweep(&roots, &heap, &references).unwrap();

        assert_eq!(garbage.cardinality(), 2);
        let expected_garbage = make_node_relation(&[3, 4]);
        assert_eq!(
            garbage.difference(&expected_garbage).unwrap().cardinality(),
            0
        );
    }

    #[test]
    fn test_mark_and_sweep_no_references() {
        let roots = make_node_relation(&[1]);
        let heap = make_node_relation(&[1, 2, 3]);
        let references = make_ref_relation(&[]);

        let garbage = mark_and_sweep(&roots, &heap, &references).unwrap();

        // Nodes 2 and 3 are garbage
        assert_eq!(garbage.cardinality(), 2);
        let expected_garbage = make_node_relation(&[2, 3]);
        assert_eq!(
            garbage.difference(&expected_garbage).unwrap().cardinality(),
            0
        );
    }
}
