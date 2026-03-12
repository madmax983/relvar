//! Relational Entity Component System (ECS).
//!
//! This module demonstrates how a relational database can be used as the backend for
//! an Entity Component System, a common architectural pattern in game development.
//!
//! # Concept
//!
//! - **Entity**: A unique integer ID.
//! - **Component**: A Relation where the first column is `entity_id`.
//! - **System**: A function that queries components (joins them on `entity_id`) and updates them.
//!
//! # Usage
//!
//! ```
//! use relvar::{InMemoryEngine, ScalarType, tuple};
//! use relvar::experimental::ecs::World;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut world = World::new(InMemoryEngine::new());
//!
//! // 1. Define Components
//! world.register_component("Position", &[("x", ScalarType::Float), ("y", ScalarType::Float)])?;
//! world.register_component("Velocity", &[("vx", ScalarType::Float), ("vy", ScalarType::Float)])?;
//!
//! // 2. Create Entity
//! let e = world.spawn()?;
//! world.add_component(e, "Position", tuple! { x: 0.0, y: 0.0 })?;
//! world.add_component(e, "Velocity", tuple! { vx: 1.0, vy: 1.0 })?;
//!
//! // 3. Run System (Update Position based on Velocity)
//! // System: For every entity with Position AND Velocity, update Position
//! world.run_update_system("Position", &["Velocity"], |joined_tuple| {
//!     let x = joined_tuple.get_typed::<f64>("x").unwrap();
//!     let y = joined_tuple.get_typed::<f64>("y").unwrap();
//!     let vx = joined_tuple.get_typed::<f64>("vx").unwrap();
//!     let vy = joined_tuple.get_typed::<f64>("vy").unwrap();
//!
//!     tuple! {
//!         x: x + vx,
//!         y: y + vy
//!     }
//! })?;
//!
//! // 4. Verify
//! let pos = world.get_component(e, "Position")?;
//! assert_eq!(pos.get_typed::<f64>("x"), Some(1.0));
//! # Ok(())
//! # }
//! ```
//!
//! # Limitations
//!
//! The `World` struct maintains the `next_entity_id` counter in memory. If the `World` instance
//! is dropped, this counter is lost. When using a persistent engine, you must manually manage
//! entity ID generation or serialize the `World` state to persist this counter.

use relvar_core::database::{Database, DatabaseError};
use relvar_core::storage_engine::StorageEngine;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{ScalarValue, Tuple};
use std::collections::HashMap;

/// A unique identifier for an entity.
pub type Entity = i64;

/// The name of the attribute used for entity IDs.
pub const ENTITY_ID_ATTR: &str = "entity_id";

/// The main container for the ECS.
pub struct World<E: StorageEngine> {
    db: Database<E>,
    next_entity_id: Entity,
}

impl<E: StorageEngine> World<E> {
    /// Creates a new World with the given storage engine.
    pub fn new(engine: E) -> Self {
        Self {
            db: Database::new(engine),
            next_entity_id: 1,
        }
    }

    /// Spawns a new entity with a unique ID.
    ///
    /// # Errors
    ///
    /// Currently, this method doesn't fail, but returns `Result` for
    /// forward-compatibility with storage engines that might track IDs persistently.
    pub fn spawn(&mut self) -> Result<Entity, DatabaseError> {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        Ok(id)
    }

    /// Registers a new component type in the underlying database.
    ///
    /// A component is stored as a relation named `C_{name}`.
    /// It automatically gets an `entity_id` column as the Primary Key.
    ///
    /// # Errors
    ///
    /// Returns a `DatabaseError` if the database fails to create the relation or set constraints,
    /// or if the component schema uses the reserved attribute `entity_id`.
    pub fn register_component(
        &mut self,
        name: &str,
        attributes: &[(&str, ScalarType)],
    ) -> Result<(), DatabaseError> {
        let rel_name = self.component_rel_name(name);

        let mut tuple_type = TupleType::new().with_attribute(ENTITY_ID_ATTR, ScalarType::Int);

        for (attr_name, attr_type) in attributes {
            if *attr_name == ENTITY_ID_ATTR {
                return Err(DatabaseError::AlgebraError(format!(
                    "Attribute name '{}' is reserved",
                    ENTITY_ID_ATTR
                )));
            }
            tuple_type = tuple_type.with_attribute(*attr_name, attr_type.clone());
        }

        let rel_type = RelationType::new(tuple_type);
        self.db.create_relvar(&rel_name, rel_type)?;

        // Set Primary Key on entity_id
        use relvar_core::constraints::{KeyConstraints, PrimaryKey};
        let pk = PrimaryKey::new(vec![ENTITY_ID_ATTR.to_string()])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let constraints = KeyConstraints::new().with_primary_key(pk);
        self.db.set_key_constraints(&rel_name, constraints)?;

        Ok(())
    }

    /// Adds a component to a specific entity.
    ///
    /// The `data` tuple must match the component schema (excluding `entity_id`).
    /// The ECS will automatically inject the `entity_id` into the tuple before
    /// inserting it into the underlying relation.
    ///
    /// # Errors
    ///
    /// Returns a `DatabaseError` if the tuple does not match the component schema,
    /// or if the underlying database insert fails.
    pub fn add_component(
        &mut self,
        entity: Entity,
        component_name: &str,
        data: Tuple,
    ) -> Result<(), DatabaseError> {
        let rel_name = self.component_rel_name(component_name);

        // Inject entity_id into the tuple
        // We need to rebuild the tuple because Tuple is immutable-ish (well, constructing one is safer)
        // But wait, Tuple internals are private. We can use `tuple!` macro or `Tuple::new`.
        // `data` has values. We extract them.

        let mut values = data.values().clone(); // Clone the BTreeMap
        values.insert(ENTITY_ID_ATTR.to_string(), ScalarValue::Int(entity));

        // Get the relation type to ensure correct schema
        let rel_type = self.db.get_relvar_type(&rel_name)?;
        let heading = rel_type.heading();

        let new_tuple = Tuple::new(heading.clone(), values)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        self.db.insert(&rel_name, new_tuple)
    }

    /// Removes a component from a specific entity.
    ///
    /// This deletes the entity's data for this component from the underlying relation.
    ///
    /// # Errors
    ///
    /// Returns a `DatabaseError` if the underlying database delete operation fails.
    pub fn remove_component(
        &mut self,
        entity: Entity,
        component_name: &str,
    ) -> Result<(), DatabaseError> {
        let rel_name = self.component_rel_name(component_name);
        self.db.delete(&rel_name, |t| {
            t.get_typed::<Entity>(ENTITY_ID_ATTR) == Some(entity)
        })?;
        Ok(())
    }

    /// Retrieves the component data for a specific entity.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::TupleMismatch` if the component is not found for the given entity,
    /// or other `DatabaseError` if the database query fails.
    pub fn get_component(
        &self,
        entity: Entity,
        component_name: &str,
    ) -> Result<Tuple, DatabaseError> {
        let rel_name = self.component_rel_name(component_name);
        let relation = self.db.query(&rel_name)?;

        relation
            .tuples()
            .find(|t| t.get_typed::<Entity>(ENTITY_ID_ATTR) == Some(entity))
            .cloned()
            .ok_or(DatabaseError::TupleMismatch) // Using TupleMismatch as "Not Found" proxy
    }

    /// Runs a system that updates a target component based on joined data.
    ///
    /// The ECS will automatically join the target component relation with all
    /// `join_components` relations on the `entity_id` attribute. The `updater`
    /// closure will be called for each resulting joined tuple.
    ///
    /// # Arguments
    ///
    /// * `target_component` - The name of the component to update.
    /// * `join_components` - Names of other components to join with.
    /// * `updater` - A function that takes the joined tuple and returns new values for the target component.
    ///   The returned tuple should NOT contain `entity_id` (it is preserved automatically).
    ///
    /// # Errors
    ///
    /// Returns a `DatabaseError` if a relation does not exist, join constraints fail,
    /// or if the updated tuple violates schema constraints.
    pub fn run_update_system<F>(
        &mut self,
        target_component: &str,
        join_components: &[&str],
        updater: F,
    ) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> Tuple,
    {
        let target_rel_name = self.component_rel_name(target_component);

        // 1. Build the query: Target JOIN C1 JOIN C2 ...
        // We start with Target.
        let mut query_result = self.db.query(&target_rel_name)?;

        for &comp_name in join_components {
            let comp_rel_name = self.component_rel_name(comp_name);
            let comp_rel = self.db.query(&comp_rel_name)?;
            query_result = query_result.join(&comp_rel)?;
        }

        // 2. Compute updates
        // We collect them into a HashMap<Entity, Tuple>
        // The updater returns the data part. We need to merge it with entity_id.
        let mut updates: HashMap<Entity, Tuple> = HashMap::new();

        // We also need the target schema to ensure the returned tuple is valid and complete
        let target_type = self.db.get_relvar_type(&target_rel_name)?;
        let target_heading = target_type.heading();

        for joined_tuple in query_result.tuples() {
            let entity_id = joined_tuple
                .get_typed::<Entity>(ENTITY_ID_ATTR)
                .ok_or(DatabaseError::AlgebraError("Missing entity_id".into()))?;

            let new_data = updater(joined_tuple);

            // Merge entity_id into new_data
            let mut values = new_data.values().clone();
            values.insert(ENTITY_ID_ATTR.to_string(), ScalarValue::Int(entity_id));

            let full_tuple = Tuple::new(target_heading.clone(), values)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            updates.insert(entity_id, full_tuple);
        }

        if updates.is_empty() {
            return Ok(0);
        }

        // 3. Apply updates
        // We use db.update with a predicate that checks existence in the updates map
        // This scans the target relation, which is O(N).
        // For each matching tuple, we replace it with the new one.

        self.db.update(
            &target_rel_name,
            |t| {
                if let Some(id) = t.get_typed::<Entity>(ENTITY_ID_ATTR) {
                    updates.contains_key(&id)
                } else {
                    false
                }
            },
            |t| {
                let id = t.get_typed::<Entity>(ENTITY_ID_ATTR).unwrap();
                updates.get(&id).unwrap().clone()
            },
        )
    }

    fn component_rel_name(&self, name: &str) -> String {
        format!("C_{}", name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::storage_engine::InMemoryEngine;
    use relvar_core::tuple;

    #[test]
    fn test_ecs_physics_system() {
        let mut world = World::new(InMemoryEngine::new());

        // 1. Define Components
        world
            .register_component(
                "Position",
                &[("x", ScalarType::Float), ("y", ScalarType::Float)],
            )
            .unwrap();
        world
            .register_component(
                "Velocity",
                &[("vx", ScalarType::Float), ("vy", ScalarType::Float)],
            )
            .unwrap();

        // 2. Create Entities
        let e1 = world.spawn().unwrap();
        world
            .add_component(e1, "Position", tuple! { x: 0.0, y: 0.0 })
            .unwrap();
        world
            .add_component(e1, "Velocity", tuple! { vx: 1.0, vy: 1.0 })
            .unwrap();

        let e2 = world.spawn().unwrap();
        world
            .add_component(e2, "Position", tuple! { x: 10.0, y: 10.0 })
            .unwrap();
        // e2 has no velocity, should not move (because join will filter it out? Or should we use Left Join?)
        // Our implementation uses Natural Join (Inner Join), so e2 will be ignored by the system.
        // This is correct for "System operates on entities WITH components A and B".

        let e3 = world.spawn().unwrap();
        world
            .add_component(e3, "Position", tuple! { x: 5.0, y: 5.0 })
            .unwrap();
        world
            .add_component(e3, "Velocity", tuple! { vx: -1.0, vy: -2.0 })
            .unwrap();

        // 3. Run System
        let count = world
            .run_update_system("Position", &["Velocity"], |joined| {
                let x = joined.get_typed::<f64>("x").unwrap();
                let y = joined.get_typed::<f64>("y").unwrap();
                let vx = joined.get_typed::<f64>("vx").unwrap();
                let vy = joined.get_typed::<f64>("vy").unwrap();

                tuple! {
                    x: x + vx,
                    y: y + vy
                }
            })
            .unwrap();

        assert_eq!(count, 2); // e1 and e3

        // 4. Verify e1
        let pos1 = world.get_component(e1, "Position").unwrap();
        assert_eq!(pos1.get_typed::<f64>("x"), Some(1.0));
        assert_eq!(pos1.get_typed::<f64>("y"), Some(1.0));

        // 5. Verify e2 (Unchanged)
        let pos2 = world.get_component(e2, "Position").unwrap();
        assert_eq!(pos2.get_typed::<f64>("x"), Some(10.0));
        assert_eq!(pos2.get_typed::<f64>("y"), Some(10.0));

        // 6. Verify e3
        let pos3 = world.get_component(e3, "Position").unwrap();
        assert_eq!(pos3.get_typed::<f64>("x"), Some(4.0));
        assert_eq!(pos3.get_typed::<f64>("y"), Some(3.0));
    }
}
