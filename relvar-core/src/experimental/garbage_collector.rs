//! Relational Mark-and-Sweep Garbage Collector.
//!
//! This module implements a Mark-and-Sweep Garbage Collector purely using relational algebra.
//! It represents roots, objects, and references as relations, and uses operations like
//! `tclose` (transitive closure), `join`, `union`, and `difference` to identify garbage.

use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// Computes the set of unreachable (garbage) objects given the roots, objects, and references.
///
/// * `roots`: A relation with at least the attribute `object_id` (Int), representing objects pointed to by roots.
/// * `objects`: A relation with at least the attribute `object_id` (Int), representing all allocated objects.
/// * `references`: A relation with exactly two attributes: `from_id` (Int) and `to_id` (Int), representing pointers between objects.
///
/// Returns a relation of the same type as `objects`, containing only the garbage objects.
pub fn sweep_garbage(
    roots: &Relation,
    objects: &Relation,
    references: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Mark phase: Find all reachable objects.

    // Get direct root objects
    let direct_roots = roots.project(&["object_id"]);

    // If there are references, compute paths
    let reached = if references.cardinality() > 0 {
        // Transitive closure of references
        let paths = references
            .tclose("from_id", "to_id")
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Find objects reachable from direct roots
        direct_roots
            .clone()
            .rename(&[("object_id", "from_id")])
            .join(&paths)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["to_id"])
            .rename(&[("to_id", "object_id")])
    } else {
        // Create an empty relation with just `object_id`
        Relation::new(RelationType::new(
            TupleType::new().with_attribute("object_id", ScalarType::Int),
        ))
    };

    // Union direct roots and reached objects
    let all_reachable = direct_roots
        .union(&reached)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Join with original objects to get full object tuples (in case there are other attributes like size)
    let live_objects = objects
        .join(&all_reachable)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 2. Sweep phase: Identify garbage by taking the difference.
    let garbage = objects
        .difference(&live_objects)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    Ok(garbage)
}
