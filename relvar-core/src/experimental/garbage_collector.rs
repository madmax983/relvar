//! Mark-and-Sweep Garbage Collector in Relational Algebra.
//!
//! This module implements a basic Mark-and-Sweep garbage collector using purely relational algebra.
//! Memory is modeled as a graph, where `roots` are the starting nodes, `heap` contains all allocated nodes,
//! and `references` contains directed edges between nodes.
//!
//! The algorithm computes reachability using transitive closure (`tclose`) and finds the set of
//! garbage nodes (unreachable memory) using set difference.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Computes the set of unreachable nodes (garbage) in a memory graph purely using relational algebra.
///
/// `roots` must have exactly one attribute: `node` (Int).
/// `heap` must have exactly one attribute: `node` (Int).
/// `references` must have two attributes: `from_node` (Int) and `to_node` (Int).
///
/// # Returns
///
/// A Relation containing the unreachable nodes (with attribute `node`).
///
/// # Examples
///
/// ```
/// use relvar_core::tuple;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar_core::experimental::garbage_collector::collect_garbage;
///
/// let node_type = RelationType::new(TupleType::new().with_attribute("node", ScalarType::Int));
/// let ref_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("from_node", ScalarType::Int)
///         .with_attribute("to_node", ScalarType::Int),
/// );
///
/// let mut roots = Relation::new(node_type.clone());
/// roots.insert(tuple! { node: 1i64 }).unwrap();
///
/// let mut heap = Relation::new(node_type);
/// heap.insert(tuple! { node: 1i64 }).unwrap();
/// heap.insert(tuple! { node: 2i64 }).unwrap();
/// heap.insert(tuple! { node: 3i64 }).unwrap();
/// heap.insert(tuple! { node: 4i64 }).unwrap(); // Unreachable cycle 4->5->4
/// heap.insert(tuple! { node: 5i64 }).unwrap();
///
/// let mut references = Relation::new(ref_type);
/// references.insert(tuple! { from_node: 1i64, to_node: 2i64 }).unwrap();
/// references.insert(tuple! { from_node: 2i64, to_node: 3i64 }).unwrap();
/// references.insert(tuple! { from_node: 4i64, to_node: 5i64 }).unwrap();
/// references.insert(tuple! { from_node: 5i64, to_node: 4i64 }).unwrap();
///
/// let garbage = collect_garbage(&roots, &heap, &references).unwrap();
///
/// // 4 and 5 are unreachable from root 1.
/// assert_eq!(garbage.cardinality(), 2);
/// assert!(garbage.contains(&tuple! { node: 4i64 }));
/// assert!(garbage.contains(&tuple! { node: 5i64 }));
/// ```
pub fn collect_garbage(
    roots: &Relation,
    heap: &Relation,
    references: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Calculate transitive closure of references.
    // This yields all pairs (from_node, to_node) where a path exists.
    // If there are no references, the closure is empty. We must handle this gracefully.
    let closure = references.tclose("from_node", "to_node")?;

    // 2. Join roots with the closure to find all nodes reachable from roots.
    // We rename from_node to node in the closure to join with roots(node).
    let closure_renamed = closure.rename(&[("from_node", "node")]);
    let reachable_paths = roots.join(&closure_renamed)?;

    // 3. Project the target nodes from the reachable paths.
    // These are the nodes that can be reached from some root.
    let reachable_descendants = reachable_paths
        .project(&["to_node"])
        .rename(&[("to_node", "node")]);

    // 4. Union the roots themselves, as roots are inherently reachable.
    let all_reachable = roots
        .union(&reachable_descendants)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 5. The garbage is the set of nodes in the heap that are not in all_reachable.
    heap.difference(&all_reachable)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_garbage_collection_simple() {
        let node_type = RelationType::new(TupleType::new().with_attribute("node", ScalarType::Int));
        let ref_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_node", ScalarType::Int)
                .with_attribute("to_node", ScalarType::Int),
        );

        let mut roots = Relation::new(node_type.clone());
        roots.insert(tuple! { node: 1i64 }).unwrap();

        let mut heap = Relation::new(node_type);
        heap.insert(tuple! { node: 1i64 }).unwrap();
        heap.insert(tuple! { node: 2i64 }).unwrap();
        heap.insert(tuple! { node: 3i64 }).unwrap();
        heap.insert(tuple! { node: 4i64 }).unwrap();

        let mut references = Relation::new(ref_type);
        references
            .insert(tuple! { from_node: 1i64, to_node: 2i64 })
            .unwrap();

        let garbage = collect_garbage(&roots, &heap, &references).unwrap();

        assert_eq!(garbage.cardinality(), 2);
        assert!(garbage.contains(&tuple! { node: 3i64 }));
        assert!(garbage.contains(&tuple! { node: 4i64 }));
    }

    #[test]
    fn test_garbage_collection_no_references() {
        let node_type = RelationType::new(TupleType::new().with_attribute("node", ScalarType::Int));
        let ref_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_node", ScalarType::Int)
                .with_attribute("to_node", ScalarType::Int),
        );

        let mut roots = Relation::new(node_type.clone());
        roots.insert(tuple! { node: 1i64 }).unwrap();

        let mut heap = Relation::new(node_type);
        heap.insert(tuple! { node: 1i64 }).unwrap();
        heap.insert(tuple! { node: 2i64 }).unwrap();

        let references = Relation::new(ref_type);

        let garbage = collect_garbage(&roots, &heap, &references).unwrap();

        assert_eq!(garbage.cardinality(), 1);
        assert!(garbage.contains(&tuple! { node: 2i64 }));
    }
}
