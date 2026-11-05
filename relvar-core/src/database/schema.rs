//! Database Schema (DDL) operations.

use crate::constraints::ConstraintManagerError;
use crate::database::Database;
use crate::database::virtual_relvar::VirtualRelvarDefinition;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use crate::types::RelationType;
use crate::values::Relation;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
impl<E: StorageEngine> Database<E> {
    /// Create a new base relvar (stored relation).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::Int)
    /// );
    ///
    /// db.create_relvar("TEST", rel_type).unwrap();
    /// assert!(db.relvar_exists("TEST"));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationAlreadyExists` if a relation with this name exists.
    ///
    /// Returns [`DatabaseError::TransactionError`] inside a transaction:
    /// schema changes are not transactional (see [`begin`](Self::begin)).
    pub fn create_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), DatabaseError> {
        self.ensure_no_transaction("create_relvar")?;
        self.ensure_complete()?;
        if self.engine.relation_exists(name) || self.virtual_relvars.contains_key(name) {
            return Err(DatabaseError::RelationAlreadyExists(name.to_string()));
        }

        self.engine.create_relation(name, relation_type)?;
        Ok(())
    }

    /// Drop a base relvar.
    ///
    /// Removes the relation variable and all its associated constraints
    /// from the database. This effectively deletes the table and all its data.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::Int)
    /// );
    ///
    /// db.create_relvar("TEST", rel_type).unwrap();
    /// assert!(db.relvar_exists("TEST"));
    ///
    /// db.drop_relvar("TEST").unwrap();
    /// assert!(!db.relvar_exists("TEST"));
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationNotFound` if the relation doesn't exist.
    ///
    /// Returns [`ConstraintManagerError::RelvarReferenced`]
    /// (naming the referencing relvar) if a foreign key of *another* relvar
    /// references this one: dropping it would leave that foreign key
    /// dangling, and recreating the relvar would orphan the referencing
    /// tuples. Drop (or redefine) the referencing foreign keys first. A
    /// relvar referenced only by its own foreign keys can be dropped.
    ///
    /// Returns [`DatabaseError::Assertion`] wrapping
    /// [`AssertionError::RelvarReferenced`](crate::constraints::AssertionError::RelvarReferenced)
    /// if a [declarative assertion](Self::add_declarative_assertion) refers
    /// to the relvar; remove the assertion first.
    ///
    /// Returns [`DatabaseError::TransactionError`] inside a transaction:
    /// schema changes are not transactional (see [`begin`](Self::begin)).
    pub fn drop_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        self.ensure_no_transaction("drop_relvar")?;
        self.ensure_complete()?;

        // A database contains its relvars *and* their constraints (RM
        // Prescription 16): refuse to strand another relvar's foreign key.
        if let Some((referencing, _)) = self
            .constraints
            .referencing_foreign_keys(name)
            .iter()
            .find(|(referencing, _)| referencing != name)
        {
            return Err(ConstraintManagerError::RelvarReferenced {
                relation: name.to_string(),
                referencing: referencing.clone(),
            }
            .into());
        }
        // Nor a declarative assertion that refers to it.
        self.ensure_not_referenced_by_assertion(name)?;

        // Drop from the engine first, so a failure leaves the constraints
        // in force. (A constraint-persisting engine forgets the relvar's
        // stored constraints as part of the drop.)
        self.engine.drop_relation(name)?;

        // Remove associated constraints and key index entries (#23), then
        // persist the reduced constraint catalog. The relvar is gone either
        // way, so its constraints go too even if that write fails.
        self.constraints.remove_constraints_for_relation(name);
        self.remove_key_index(name);
        self.engine
            .store_constraints(&self.constraints.to_catalog())?;
        Ok(())
    }

    /// Check if a relvar exists (base or virtual).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    ///
    /// db.create_relvar("TEST", rel_type).unwrap();
    /// assert!(db.relvar_exists("TEST"));
    /// assert!(!db.relvar_exists("MISSING"));
    /// ```
    pub fn relvar_exists(&self, name: &str) -> bool {
        self.engine.relation_exists(name) || self.virtual_relvars.contains_key(name)
    }

    /// List all relvar names (base and virtual).
    ///
    /// This is useful for building database inspection tools (like the visualizer)
    /// or for exploring an unfamiliar database schema. It combines both physically
    /// stored relvars and dynamically computed virtual relvars.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    ///
    /// db.create_relvar("TABLE_1", rel_type.clone()).unwrap();
    /// db.define_virtual_relvar("VIEW_1", rel_type, |db_exec| {
    ///     Ok(relvar_core::values::Relation::new(relvar_core::types::RelationType::new(relvar_core::types::TupleType::new())))
    /// }).unwrap();
    ///
    /// let mut relvars = db.list_relvars();
    /// relvars.sort();
    /// assert_eq!(relvars, vec!["TABLE_1", "VIEW_1"]);
    /// ```
    pub fn list_relvars(&self) -> Vec<String> {
        let mut names = self.engine.list_relations();
        names.extend(self.virtual_relvars.keys().cloned());
        names
    }

    /// Get the relation type (heading) for a relvar.
    ///
    /// This method retrieves the metadata without loading the full relation.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationNotFound` if the relvar doesn't exist.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// let rel_type = RelationType::new(heading);
    /// db.create_relvar("USERS", rel_type.clone()).unwrap();
    ///
    /// let fetched_type = db.get_relvar_type("USERS").unwrap();
    /// assert_eq!(fetched_type.degree(), 1);
    /// ```
    pub fn get_relvar_type(&self, name: &str) -> Result<RelationType, DatabaseError> {
        // Check virtual relvars first
        if let Some(def) = self.virtual_relvars.get(name) {
            return Ok(def.relation_type.clone());
        }

        // Check base relvars
        let metadata = self.engine.get_relation_metadata(name)?;
        Ok(metadata.relation_type)
    }

    ///
    /// Defines a virtual relvar (view).
    /// A virtual relvar (or view) acts like a regular relation but is not stored
    /// on disk. Its contents are dynamically generated by evaluating a given function
    /// every time it is queried.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let emp_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::Int)
    ///         .with_attribute("active", ScalarType::Bool)
    /// );
    /// db.create_relvar("EMP", emp_type.clone()).unwrap();
    /// db.insert("EMP", tuple! { id: 1i64, active: true }).unwrap();
    /// db.insert("EMP", tuple! { id: 2i64, active: false }).unwrap();
    ///
    /// // Define a view for active employees
    /// db.define_virtual_relvar(
    ///     "ACTIVE_EMP",
    ///     emp_type,
    ///     |db_exec| {
    ///         let emp = db_exec.query("EMP").unwrap();
    ///         Ok(emp.restrict(|t| t.get_typed::<bool>("active").unwrap_or(false)))
    ///     }
    /// ).unwrap();
    ///
    /// let active_emps = db.query("ACTIVE_EMP").unwrap();
    /// assert_eq!(active_emps.cardinality(), 1);
    /// ```
    ///
    /// # Errors
    ///
    /// Yields an error if a relvar with this name already exists.
    pub fn define_virtual_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
        evaluator: fn(&Database<E>) -> Result<Relation, DatabaseError>,
    ) -> Result<(), DatabaseError> {
        if self.relvar_exists(name) {
            return Err(DatabaseError::RelationAlreadyExists(name.to_string()));
        }

        self.virtual_relvars.insert(
            name.to_string(),
            VirtualRelvarDefinition {
                relation_type,
                evaluator,
            },
        );

        Ok(())
    }

    /// Drop a virtual relvar.
    ///
    /// Removes the definition of the virtual relvar from the database.
    /// This does not delete any underlying data since virtual relvars
    /// are not stored.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    ///
    /// db.define_virtual_relvar("MY_VIEW", rel_type, |db_exec| {
    ///     Ok(relvar_core::values::Relation::new(relvar_core::types::RelationType::new(relvar_core::types::TupleType::new())))
    /// }).unwrap();
    ///
    /// assert!(db.relvar_exists("MY_VIEW"));
    /// db.drop_virtual_relvar("MY_VIEW").unwrap();
    /// assert!(!db.relvar_exists("MY_VIEW"));
    /// ```
    ///
    /// # Errors
    ///
    /// Yields an error if the virtual relvar doesn't exist.
    pub fn drop_virtual_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        self.virtual_relvars
            .remove(name)
            .ok_or_else(|| DatabaseError::RelationNotFound(name.to_string()))?;
        Ok(())
    }
}
