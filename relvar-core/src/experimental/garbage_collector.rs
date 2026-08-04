//! Mark-and-Sweep Garbage Collection in Relational Algebra.
//!
//! This module implements a Mark-and-Sweep GC algorithm using solely relational operators.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Evaluates Mark-and-Sweep Garbage Collection using purely relational algebra.
///
/// `heap` must have attributes: `id` (Int).
/// `roots` must have attributes: `id` (Int).
/// `references` must have attributes: `from_id` (Int), `to_id` (Int).
///
/// Returns a `Relation` containing the ids of the garbage objects.
///
/// # Examples
/// ```
/// use relvar_core::tuple;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar_core::experimental::garbage_collector::mark_and_sweep;
///
/// let id_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
/// let ref_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("from_id", ScalarType::Int)
///         .with_attribute("to_id", ScalarType::Int)
/// );
///
/// let mut heap = Relation::new(id_type.clone());
/// let mut roots = Relation::new(id_type.clone());
/// let mut refs = Relation::new(ref_type);
///
/// heap.insert(tuple! { id: 1i64 }).unwrap();
/// heap.insert(tuple! { id: 2i64 }).unwrap();
/// heap.insert(tuple! { id: 3i64 }).unwrap(); // Garbage
///
/// roots.insert(tuple! { id: 1i64 }).unwrap();
/// refs.insert(tuple! { from_id: 1i64, to_id: 2i64 }).unwrap();
///
/// let garbage = mark_and_sweep(&heap, &roots, &refs).unwrap();
/// assert_eq!(garbage.cardinality(), 1);
/// let g_tuple = garbage.tuples().next().unwrap();
/// assert_eq!(g_tuple.get_typed::<i64>("id").unwrap(), 3i64);
/// ```
pub fn mark_and_sweep(
    heap: &Relation,
    roots: &Relation,
    references: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Mark phase: Compute all reachable nodes via transitive closure from roots
    let closure = references.tclose("from_id", "to_id")?;

    // Nodes reachable from roots: join roots with closure on 'id' -> 'from_id'
    let reachable_from_roots = roots
        .clone()
        .rename_into(&[("id", "from_id")])
        .join(&closure)?
        .project(&["to_id"])
        .rename_into(&[("to_id", "id")]);

    // The roots themselves are also reachable
    let all_reachable = roots
        .clone()
        .union(&reachable_from_roots)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 2. Sweep phase: Find garbage by taking the difference of heap and reachable nodes
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

    #[test]
    fn test_mark_and_sweep() {
        let id_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        let ref_type = RelationType::new(
            TupleType::new()
                .with_attribute("from_id", ScalarType::Int)
                .with_attribute("to_id", ScalarType::Int),
        );

        let mut heap = Relation::new(id_type.clone());
        let mut roots = Relation::new(id_type.clone());
        let mut refs = Relation::new(ref_type);

        for id in 1..=6 {
            heap.insert(tuple! { id: id as i64 }).unwrap();
        }

        roots.insert(tuple! { id: 1i64 }).unwrap();
        roots.insert(tuple! { id: 2i64 }).unwrap();

        refs.insert(tuple! { from_id: 1i64, to_id: 3i64 }).unwrap();
        refs.insert(tuple! { from_id: 3i64, to_id: 4i64 }).unwrap();
        refs.insert(tuple! { from_id: 5i64, to_id: 6i64 }).unwrap(); // 5 and 6 are disconnected from roots

        let garbage = mark_and_sweep(&heap, &roots, &refs).unwrap();

        assert_eq!(garbage.cardinality(), 2);

        let mut garbage_ids: Vec<i64> = garbage
            .tuples()
            .map(|t| t.get_typed::<i64>("id").unwrap())
            .collect();
        garbage_ids.sort();

        assert_eq!(garbage_ids, vec![5, 6]);
    }
}
