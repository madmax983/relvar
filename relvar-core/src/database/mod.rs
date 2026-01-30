//! Core database instance and operations.
//!
//! This module provides the [`Database`] struct, which is the main entry point
//! for all database operations.

use crate::constraints::{
    AttributeConstraints, CheckConstraints, ForeignKeyConstraints, KeyConstraints,
};
use crate::storage_engine::StorageEngine;
use crate::types::RelationType;
use crate::values::{Relation, Tuple};

/// Database errors.
pub mod error;
pub use error::DatabaseError;

/// Database traits.
pub mod traits;
pub use traits::QueryContext;

/// System catalog.
pub mod catalog;
pub use catalog::{SystemCatalog, VirtualRelvarDefinition};

#[cfg(test)]
mod tests;

/// A relational database instance.
///
/// Generic over the storage engine `E`, which handles persistence.
pub struct Database<E: StorageEngine> {
    /// Storage engine for persisting relations.
    engine: E,
    /// System catalog managing metadata and constraints.
    catalog: SystemCatalog,
    /// Whether a transaction is currently in progress.
    in_transaction: bool,
    /// Transaction savepoint.
    transaction_snapshot: Option<E::Snapshot>,
}

impl<E: StorageEngine> QueryContext for Database<E> {
    fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError> {
        // Check if this is a virtual relvar
        if self.catalog.is_virtual(relation_name) {
            let def = self
                .catalog
                .get_virtual_relvar(relation_name)
                .unwrap()
                .clone();
            return (def.evaluator)(self);
        }

        // Otherwise, load from engine
        Ok(self.engine.load_relation(relation_name)?)
    }

    fn relation_type(&self, relation_name: &str) -> Result<RelationType, DatabaseError> {
        if let Some(def) = self.catalog.get_virtual_relvar(relation_name) {
            Ok(def.relation_type.clone())
        } else {
            Ok(self
                .engine
                .get_relation_metadata(relation_name)?
                .relation_type)
        }
    }
}

impl<E: StorageEngine> Database<E> {
    /// Create a new database with the given storage engine.
    pub fn new(engine: E) -> Self {
        Self {
            engine,
            catalog: SystemCatalog::new(),
            in_transaction: false,
            transaction_snapshot: None,
        }
    }

    /// Create a new base relvar (stored relation).
    pub fn create_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), DatabaseError> {
        if self.engine.relation_exists(name) || self.catalog.is_virtual(name) {
            return Err(DatabaseError::RelationAlreadyExists(name.to_string()));
        }

        self.engine.create_relation(name, relation_type)?;
        Ok(())
    }

    /// Drop a base relvar.
    pub fn drop_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        // Remove associated constraints
        self.catalog.drop_relvar_metadata(name);

        // Drop from engine
        self.engine.drop_relation(name)?;
        Ok(())
    }

    /// Check if a relvar exists (base or virtual).
    pub fn relvar_exists(&self, name: &str) -> bool {
        self.engine.relation_exists(name) || self.catalog.is_virtual(name)
    }

    /// List all relvar names (base and virtual).
    pub fn list_relvars(&self) -> Vec<String> {
        let mut names = self.engine.list_relations();
        names.extend(self.catalog.virtual_relvar_names().cloned());
        names
    }

    /// Set key constraints for a relation.
    pub fn set_key_constraints(
        &mut self,
        relation_name: &str,
        constraints: KeyConstraints,
    ) -> Result<(), DatabaseError> {
        if !self.engine.relation_exists(relation_name) {
            return Err(DatabaseError::RelationNotFound(relation_name.to_string()));
        }
        self.catalog
            .set_key_constraints(&self.engine, relation_name, constraints)
    }

    /// Set foreign key constraints for a relation.
    pub fn set_foreign_key_constraints(
        &mut self,
        relation_name: &str,
        constraints: ForeignKeyConstraints,
    ) -> Result<(), DatabaseError> {
        if !self.engine.relation_exists(relation_name) {
            return Err(DatabaseError::RelationNotFound(relation_name.to_string()));
        }
        self.catalog
            .set_foreign_key_constraints(&self.engine, relation_name, constraints)
    }

    /// Set type constraints for an attribute.
    pub fn set_type_constraints(
        &mut self,
        relation_name: &str,
        attribute_name: &str,
        constraints: AttributeConstraints,
    ) -> Result<(), DatabaseError> {
        self.catalog
            .set_type_constraints(&self.engine, relation_name, attribute_name, constraints)
    }

    /// Set CHECK constraints for a relation.
    pub fn set_check_constraints(
        &mut self,
        relation_name: &str,
        constraints: CheckConstraints,
    ) -> Result<(), DatabaseError> {
        if !self.engine.relation_exists(relation_name) {
            return Err(DatabaseError::RelationNotFound(relation_name.to_string()));
        }
        self.catalog
            .set_check_constraints(&self.engine, relation_name, constraints)
    }

    /// Insert a tuple into a relation.
    pub fn insert(&mut self, relation_name: &str, tuple: Tuple) -> Result<(), DatabaseError> {
        self.catalog.validate_insert(self, relation_name, &tuple)?;
        self.engine.insert_tuple(relation_name, tuple)?;
        Ok(())
    }

    /// Query a relation (base or virtual).
    pub fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError> {
        QueryContext::query(self, relation_name)
    }

    /// Delete tuples matching a predicate.
    pub fn delete<F>(&mut self, relation_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
    {
        self.catalog.ensure_not_virtual(relation_name)?;

        // Load current relation
        let current_relation = self.query(relation_name)?;

        // Filter out tuples to delete
        let mut new_relation = Relation::new(current_relation.relation_type().clone());
        let mut delete_count = 0;

        for tuple in current_relation.tuples() {
            if predicate(tuple) {
                delete_count += 1;
            } else {
                new_relation.insert(tuple.clone())?;
            }
        }

        self.catalog
            .validate_delete(self, relation_name, &new_relation)?;

        // Store the new relation
        self.engine.store_relation(relation_name, &new_relation)?;
        Ok(delete_count)
    }

    /// Update tuples matching a predicate.
    pub fn update<F, U>(
        &mut self,
        relation_name: &str,
        predicate: F,
        updater: U,
    ) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
        U: Fn(&Tuple) -> Tuple,
    {
        self.catalog.ensure_not_virtual(relation_name)?;

        // Load current relation
        let current_relation = self.query(relation_name)?;

        // Apply updates
        let mut new_relation = Relation::new(current_relation.relation_type().clone());
        let mut update_count = 0;

        for tuple in current_relation.tuples() {
            if predicate(tuple) {
                let updated_tuple = updater(tuple);

                // Validate updated tuple
                if !updated_tuple.conforms_to(current_relation.relation_type().tuple_type()) {
                    return Err(DatabaseError::TupleMismatch);
                }

                new_relation.insert(updated_tuple)?;
                update_count += 1;
            } else {
                new_relation.insert(tuple.clone())?;
            }
        }

        self.catalog.validate_update(relation_name, &new_relation)?;

        // Store the new relation
        self.engine.store_relation(relation_name, &new_relation)?;
        Ok(update_count)
    }

    /// Begin a transaction.
    pub fn begin(&mut self) -> Result<(), DatabaseError> {
        if self.in_transaction {
            return Err(DatabaseError::TransactionError(
                "Transaction already in progress".to_string(),
            ));
        }

        let snapshot = self.engine.begin_transaction()?;
        self.transaction_snapshot = Some(snapshot);
        self.in_transaction = true;
        Ok(())
    }

    /// Commit the current transaction.
    pub fn commit(&mut self) -> Result<(), DatabaseError> {
        if !self.in_transaction {
            return Err(DatabaseError::TransactionError(
                "No transaction in progress".to_string(),
            ));
        }

        if let Some(snapshot) = self.transaction_snapshot.take() {
            self.engine.commit_transaction(snapshot)?;
        }

        self.in_transaction = false;
        Ok(())
    }

    /// Rollback the current transaction.
    pub fn rollback(&mut self) -> Result<(), DatabaseError> {
        if !self.in_transaction {
            return Err(DatabaseError::TransactionError(
                "No transaction in progress".to_string(),
            ));
        }

        if let Some(snapshot) = self.transaction_snapshot.take() {
            self.engine.rollback_transaction(snapshot)?;
        }

        self.in_transaction = false;
        Ok(())
    }

    /// Define a virtual relvar (view).
    pub fn define_virtual_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
        evaluator: fn(&dyn QueryContext) -> Result<Relation, DatabaseError>,
    ) -> Result<(), DatabaseError> {
        if self.relvar_exists(name) {
            return Err(DatabaseError::RelationAlreadyExists(name.to_string()));
        }

        self.catalog
            .define_virtual_relvar(name, relation_type, evaluator)
    }

    /// Drop a virtual relvar.
    pub fn drop_virtual_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        self.catalog.drop_virtual_relvar(name)
    }
}
