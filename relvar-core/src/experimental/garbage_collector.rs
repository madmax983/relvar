//! Relational Garbage Collector.
//!
//! This module implements a Mark-and-Sweep Garbage Collector purely using relational algebra.
//! It represents memory as relations and computes reachability and garbage using relational operations.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Evaluates a Mark-and-Sweep Garbage Collector cycle using pure relational algebra.
///
/// `roots` must have attribute: `node` (Int).
/// `heap` must have attribute: `node` (Int).
/// `references` must have attributes: `source` (Int), `target` (Int).
///
/// Returns a `Relation` containing the nodes that are unreachable from the roots (the garbage),
/// with attribute: `node` (Int).
///
/// # Examples
/// ```
/// use relvar_core::tuple;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar_core::experimental::garbage_collector::mark_and_sweep;
///
/// // Setup heap: nodes 1, 2, 3, 4, 5
/// let node_type = RelationType::new(TupleType::new().with_attribute("node", ScalarType::Int));
/// let mut heap = Relation::new(node_type.clone());
/// for i in 1..=5 { heap.insert(tuple! { node: i as i64 }).unwrap(); }
///
/// // Setup roots: node 1
/// let mut roots = Relation::new(node_type.clone());
/// roots.insert(tuple! { node: 1i64 }).unwrap();
///
/// // Setup references: 1 -> 2, 2 -> 3, 4 -> 5
/// let ref_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("source", ScalarType::Int)
///         .with_attribute("target", ScalarType::Int)
/// );
/// let mut references = Relation::new(ref_type);
/// references.insert(tuple! { source: 1i64, target: 2i64 }).unwrap();
/// references.insert(tuple! { source: 2i64, target: 3i64 }).unwrap();
/// references.insert(tuple! { source: 4i64, target: 5i64 }).unwrap();
///
/// // Run GC
/// let garbage = mark_and_sweep(&roots, &heap, &references).unwrap();
///
/// // Nodes 4 and 5 are unreachable from root 1
/// assert_eq!(garbage.cardinality(), 2);
/// assert!(garbage.contains(&tuple! { node: 4i64 }));
/// assert!(garbage.contains(&tuple! { node: 5i64 }));
/// ```
pub fn mark_and_sweep(
    roots: &Relation,
    heap: &Relation,
    references: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Mark phase: Compute reachability
    // Get all transitive paths in the reference graph
    let paths = references.tclose("source", "target")?;

    // Find all nodes reachable from the roots
    let roots_as_source = roots.rename(&[("node", "source")]);
    let reachable_from_roots = roots_as_source.join(&paths)?;
    let indirectly_reachable = reachable_from_roots
        .project(&["target"])
        .rename(&[("target", "node")]);

    // The full set of reachable nodes includes the roots themselves and anything they can reach
    let reachable = roots
        .union(&indirectly_reachable)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 2. Sweep phase: Identify garbage
    // Garbage is simply the heap MINUS the reachable nodes
    heap.difference(&reachable)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_mark_and_sweep() {
        let node_type = RelationType::new(TupleType::new().with_attribute("node", ScalarType::Int));
        let mut heap = Relation::new(node_type.clone());
        for i in 1..=6 {
            heap.insert(tuple! { node: i as i64 }).unwrap();
        }

        let mut roots = Relation::new(node_type.clone());
        roots.insert(tuple! { node: 1i64 }).unwrap();
        roots.insert(tuple! { node: 6i64 }).unwrap();

        let ref_type = RelationType::new(
            TupleType::new()
                .with_attribute("source", ScalarType::Int)
                .with_attribute("target", ScalarType::Int),
        );
        let mut references = Relation::new(ref_type);
        // 1 -> 2 -> 3
        references
            .insert(tuple! { source: 1i64, target: 2i64 })
            .unwrap();
        references
            .insert(tuple! { source: 2i64, target: 3i64 })
            .unwrap();
        // 4 -> 5 (Unreachable cycle or component)
        references
            .insert(tuple! { source: 4i64, target: 5i64 })
            .unwrap();
        references
            .insert(tuple! { source: 5i64, target: 4i64 })
            .unwrap();
        // 6 has no outbound references, just a root

        let garbage = mark_and_sweep(&roots, &heap, &references).unwrap();

        assert_eq!(garbage.cardinality(), 2);
        assert!(garbage.contains(&tuple! { node: 4i64 }));
        assert!(garbage.contains(&tuple! { node: 5i64 }));
    }
}
