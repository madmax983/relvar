//! Relational Mark-and-Sweep Garbage Collector
//!
//! This module demonstrates how a Mark-and-Sweep Garbage Collector can be implemented
//! using purely relational algebra operations. The heap, pointers, and root set are
//! represented as relations. Reachability is computed declaratively using transitive
//! closure (`tclose`), and garbage collection is performed using set difference.
//!
//! # Concept
//!
//! - **Objects**: Relation `(id: Int, size: Int)` representing allocated memory objects.
//! - **Pointers**: Relation `(from_id: Int, to_id: Int)` representing references between objects.
//! - **Roots**: Relation `(id: Int)` representing the root set (e.g., variables on the stack).
//!
//! The Mark-and-Sweep algorithm:
//! 1. **Mark**: Compute the transitive closure of `pointers` using `tclose`. Join this with
//!    `roots` to find all transitively reachable objects. Union these with the `roots` themselves.
//! 2. **Sweep**: The live objects are the reachable ones. Garbage is the set difference between
//!    all `objects` and the live objects. We then restrict the `objects` and `pointers` relations
//!    to only contain live objects.

use relvar_core::{
    error::DatabaseError,

    values::Relation,
};

/// A Relational Mark-and-Sweep Garbage Collector.
pub struct GarbageCollector {
    /// The heap objects. Schema: `(id: Int, size: Int)`
    pub objects: Relation,
    /// The pointers between objects. Schema: `(from_id: Int, to_id: Int)`
    pub pointers: Relation,
    /// The root set. Schema: `(id: Int)`
    pub roots: Relation,
}

impl GarbageCollector {
    /// Creates a new GarbageCollector.
    pub fn new(objects: Relation, pointers: Relation, roots: Relation) -> Self {
        Self {
            objects,
            pointers,
            roots,
        }
    }

    /// Computes the set of reachable object IDs.
    /// Returns a relation with heading `(id: Int)`.
    pub fn compute_reachable(&self) -> Result<Relation, DatabaseError> {
        // Compute transitive closure of pointers
        let closure = self.pointers.tclose("from_id", "to_id")?;

        // Find objects reachable from roots: roots ⨝ closure (join on roots.id = closure.from_id)
        let root_renamed = self.roots.rename(&[("id", "from_id")]);
        let reachable_from_roots = root_renamed
            .join(&closure)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Project just the to_id, and rename to id
        let reachable_descendants = reachable_from_roots
            .project(&["to_id"])
            .rename(&[("to_id", "id")]);

        // Total reachable = roots ∪ reachable_descendants
        let all_reachable = self
            .roots
            .union(&reachable_descendants)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(all_reachable)
    }

    /// Computes the garbage (unreachable) objects.
    /// Returns a relation with heading `(id: Int, size: Int)`.
    pub fn compute_garbage(&self) -> Result<Relation, DatabaseError> {
        let reachable_ids = self.compute_reachable()?;
        let reachable_objects = self
            .objects
            .join(&reachable_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Garbage = All Objects - Reachable Objects
        let garbage = self
            .objects
            .difference(&reachable_objects)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(garbage)
    }

    /// Performs the sweep phase, updating `objects` and `pointers` to only
    /// contain live (reachable) objects.
    pub fn collect_garbage(&mut self) -> Result<(), DatabaseError> {
        let reachable_ids = self.compute_reachable()?;

        // Keep only objects that are in reachable_ids
        self.objects = self
            .objects
            .join(&reachable_ids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Keep only pointers where from_id is reachable
        let reachable_from = reachable_ids.rename(&[("id", "from_id")]);
        self.pointers = self
            .pointers
            .join(&reachable_from)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Keep only pointers where to_id is reachable (just in case, though unreachable objects shouldn't have incoming pointers from reachable ones)
        let reachable_to = reachable_ids.rename(&[("id", "to_id")]);
        self.pointers = self
            .pointers
            .join(&reachable_to)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    fn setup_gc() -> GarbageCollector {
        let obj_type = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("size", ScalarType::Int);
        let mut objects = Relation::new(RelationType::new(obj_type));

        let ptr_type = TupleType::new()
            .with_attribute("from_id", ScalarType::Int)
            .with_attribute("to_id", ScalarType::Int);
        let mut pointers = Relation::new(RelationType::new(ptr_type));

        let root_type = TupleType::new().with_attribute("id", ScalarType::Int);
        let mut roots = Relation::new(RelationType::new(root_type));

        // Objects: 1, 2, 3, 4, 5
        objects.insert(tuple! { id: 1i64, size: 10i64 }).unwrap();
        objects.insert(tuple! { id: 2i64, size: 20i64 }).unwrap();
        objects.insert(tuple! { id: 3i64, size: 30i64 }).unwrap();
        objects.insert(tuple! { id: 4i64, size: 40i64 }).unwrap();
        objects.insert(tuple! { id: 5i64, size: 50i64 }).unwrap();

        // Roots: 1
        roots.insert(tuple! { id: 1i64 }).unwrap();

        // Pointers:
        // 1 -> 2
        // 2 -> 3
        // 4 -> 5 (Unreachable cycle/chain)
        pointers
            .insert(tuple! { from_id: 1i64, to_id: 2i64 })
            .unwrap();
        pointers
            .insert(tuple! { from_id: 2i64, to_id: 3i64 })
            .unwrap();
        pointers
            .insert(tuple! { from_id: 4i64, to_id: 5i64 })
            .unwrap();

        GarbageCollector::new(objects, pointers, roots)
    }

    #[test]
    fn test_compute_reachable() {
        let gc = setup_gc();
        let reachable = gc.compute_reachable().unwrap();
        assert_eq!(reachable.cardinality(), 3);

        let id_1 = tuple! { id: 1i64 };
        let id_2 = tuple! { id: 2i64 };
        let id_3 = tuple! { id: 3i64 };
        let id_4 = tuple! { id: 4i64 };

        assert!(reachable.contains(&id_1));
        assert!(reachable.contains(&id_2));
        assert!(reachable.contains(&id_3));
        assert!(!reachable.contains(&id_4));
    }

    #[test]
    fn test_compute_garbage() {
        let gc = setup_gc();
        let garbage = gc.compute_garbage().unwrap();
        assert_eq!(garbage.cardinality(), 2);

        let obj_4 = tuple! { id: 4i64, size: 40i64 };
        let obj_5 = tuple! { id: 5i64, size: 50i64 };
        let obj_3 = tuple! { id: 3i64, size: 30i64 };

        assert!(garbage.contains(&obj_4));
        assert!(garbage.contains(&obj_5));
        assert!(!garbage.contains(&obj_3));
    }

    #[test]
    fn test_collect_garbage() {
        let mut gc = setup_gc();
        gc.collect_garbage().unwrap();

        assert_eq!(gc.objects.cardinality(), 3);
        assert_eq!(gc.pointers.cardinality(), 2);

        // Ensure 4 and 5 are gone
        let obj_4 = tuple! { id: 4i64, size: 40i64 };
        assert!(!gc.objects.contains(&obj_4));

        // Ensure ptr 4 -> 5 is gone
        let ptr_4_5 = tuple! { from_id: 4i64, to_id: 5i64 };
        assert!(!gc.pointers.contains(&ptr_4_5));
    }
}
