//! Core database instance and operations.
//!
//! This module provides the [`Database`] struct, which is the main entry point
//! for all database operations.

use crate::constraints::{AttributeConstraints, ForeignKeyConstraints, KeyConstraints};
use crate::storage_engine::{StorageEngine, StorageError};
use crate::types::RelationType;
use crate::values::relation::RelationError;
use crate::values::{Relation, ScalarValue, Tuple};

use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during database operations.
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// A storage error occurred.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// An error occurred with a relation value.
    #[error("Relation error: {0}")]
    Relation(#[from] RelationError),

    /// Attempted to create a relation that already exists.
    #[error("Relation {0} already exists")]
    RelationAlreadyExists(String),

    /// The specified relation does not exist.
    #[error("Relation {0} not found")]
    RelationNotFound(String),

    /// The tuple's type does not match the relation's heading.
    #[error("Tuple type does not match relation type")]
    TupleMismatch,

    /// Primary key constraint violation.
    #[error("Primary key violation")]
    PrimaryKeyViolation,

    /// Candidate key constraint violation.
    #[error("Candidate key violation")]
    CandidateKeyViolation,

    /// Foreign key constraint violation.
    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    /// Type constraint violation.
    #[error("Type constraint violation: {0}")]
    TypeConstraintViolation(String),

    /// Transaction error.
    #[error("Transaction error: {0}")]
    TransactionError(String),

    /// Cannot modify a virtual relvar.
    #[error("Cannot modify virtual relvar {0}")]
    CannotModifyVirtualRelvar(String),

    /// Cannot drop a system relvar.
    #[error("Cannot drop system relvar {0}")]
    CannotDropSystemRelvar(String),

    /// Duplicate attribute name.
    #[error("Duplicate attribute name: {0}")]
    DuplicateAttributeName(String),

    /// The attribute does not exist.
    #[error("Attribute {0} not found in relation {1}")]
    AttributeNotFound(String, String),
}

/// Definition of a virtual relvar (view).
///
/// TTM: RM Prescription 10 - Virtual relvars (views) re-evaluate their
/// defining expression on each query.
#[derive(Debug, Clone)]
pub struct VirtualRelvarDefinition<E: StorageEngine> {
    /// The name of the virtual relvar.
    pub name: String,
    /// The relation type (heading).
    pub relation_type: RelationType,
    /// The evaluation function that computes the virtual relvar's contents.
    ///
    /// Takes a mutable reference to the database and returns the computed relation.
    pub evaluator: fn(&mut Database<E>) -> Result<Relation, DatabaseError>,
}

/// A relational database instance.
///
/// Generic over the storage engine `E`, which handles persistence.
///
/// # Example
///
/// ```
/// use relvar_core::database::Database;
/// use relvar_core::storage_engine::InMemoryEngine;
/// use relvar_core::types::{TupleType, RelationType, ScalarType};
/// use relvar_core::tuple;
///
/// let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
///
/// let rel_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id", ScalarType::Int)
///         .with_attribute("name", ScalarType::String)
/// );
///
/// db.create_relvar("EMPLOYEES", rel_type).unwrap();
/// db.insert("EMPLOYEES", tuple! { id: 1i64, name: "Alice" }).unwrap();
///
/// let employees = db.query("EMPLOYEES").unwrap();
/// assert_eq!(employees.cardinality(), 1);
/// ```
pub struct Database<E: StorageEngine> {
    /// Storage engine for persisting relations.
    engine: E,
    /// Key constraints (primary and candidate) per relation.
    key_constraints: HashMap<String, KeyConstraints>,
    /// Foreign key constraints per relation.
    foreign_key_constraints: HashMap<String, ForeignKeyConstraints>,
    /// Type constraints per relation, per attribute.
    type_constraints: HashMap<String, HashMap<String, AttributeConstraints>>,
    /// Whether a transaction is currently in progress.
    in_transaction: bool,
    /// Transaction savepoint.
    transaction_snapshot: Option<crate::storage_engine::TransactionSnapshot>,
    /// Virtual relvars defined by expressions.
    virtual_relvars: HashMap<String, VirtualRelvarDefinition<E>>,
}

impl<E: StorageEngine> Database<E> {
    /// Create a new database with the given storage engine.
    pub fn new(engine: E) -> Self {
        Self {
            engine,
            key_constraints: HashMap::new(),
            foreign_key_constraints: HashMap::new(),
            type_constraints: HashMap::new(),
            in_transaction: false,
            transaction_snapshot: None,
            virtual_relvars: HashMap::new(),
        }
    }

    /// Create a new base relvar (stored relation).
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationAlreadyExists` if a relation with this name exists.
    pub fn create_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
    ) -> Result<(), DatabaseError> {
        if self.engine.relation_exists(name) || self.virtual_relvars.contains_key(name) {
            return Err(DatabaseError::RelationAlreadyExists(name.to_string()));
        }

        self.engine.create_relation(name, relation_type)?;
        Ok(())
    }

    /// Drop a base relvar.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationNotFound` if the relation doesn't exist.
    pub fn drop_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        // Remove associated constraints
        self.key_constraints.remove(name);
        self.foreign_key_constraints.remove(name);
        self.type_constraints.remove(name);

        // Drop from engine
        self.engine.drop_relation(name)?;
        Ok(())
    }

    /// Check if a relvar exists (base or virtual).
    pub fn relvar_exists(&self, name: &str) -> bool {
        self.engine.relation_exists(name) || self.virtual_relvars.contains_key(name)
    }

    /// List all relvar names (base and virtual).
    pub fn list_relvars(&self) -> Vec<String> {
        let mut names = self.engine.list_relations();
        names.extend(self.virtual_relvars.keys().cloned());
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

        // Validate constraints against existing data
        let relation = self.query(relation_name)?;

        if let Some(pk) = constraints.primary_key() {
            for tuple in relation.tuples() {
                if pk
                    .would_violate(&relation, tuple)
                    .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
                {
                    return Err(DatabaseError::PrimaryKeyViolation);
                }
            }
        }

        for ck in constraints.candidate_keys() {
            for tuple in relation.tuples() {
                if ck
                    .would_violate(&relation, tuple)
                    .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
                {
                    return Err(DatabaseError::CandidateKeyViolation);
                }
            }
        }

        self.key_constraints
            .insert(relation_name.to_string(), constraints);
        Ok(())
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

        // Validate constraints against existing data
        let relation = self.query(relation_name)?;

        for fk in constraints.foreign_keys() {
            let referenced_relation = self.query(fk.referenced_relation_name())?;

            for tuple in relation.tuples() {
                if fk
                    .would_violate_on_insert(tuple, &referenced_relation)
                    .map_err(|e| DatabaseError::ForeignKeyViolation(e.to_string()))?
                {
                    return Err(DatabaseError::ForeignKeyViolation(
                        "Existing tuple violates foreign key".to_string(),
                    ));
                }
            }
        }

        self.foreign_key_constraints
            .insert(relation_name.to_string(), constraints);
        Ok(())
    }

    /// Set type constraints for an attribute.
    pub fn set_type_constraints(
        &mut self,
        relation_name: &str,
        attribute_name: &str,
        constraints: AttributeConstraints,
    ) -> Result<(), DatabaseError> {
        let metadata = self.engine.get_relation_metadata(relation_name)?;

        if !metadata.relation_type.has_attribute(attribute_name) {
            return Err(DatabaseError::AttributeNotFound(
                attribute_name.to_string(),
                relation_name.to_string(),
            ));
        }

        // Validate constraints against existing data
        let relation = self.query(relation_name)?;

        for tuple in relation.tuples() {
            if let Some(value) = tuple.get(attribute_name)
                && !constraints
                    .is_satisfied_by(value)
                    .map_err(|e| DatabaseError::TypeConstraintViolation(e.to_string()))?
            {
                return Err(DatabaseError::TypeConstraintViolation(
                    "Existing value violates constraint".to_string(),
                ));
            }
        }

        self.type_constraints
            .entry(relation_name.to_string())
            .or_default()
            .insert(attribute_name.to_string(), constraints);
        Ok(())
    }

    /// Insert a tuple into a relation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The relation doesn't exist
    /// - The tuple type doesn't match
    /// - A constraint is violated
    pub fn insert(&mut self, relation_name: &str, tuple: Tuple) -> Result<(), DatabaseError> {
        // Check if trying to insert into a virtual relvar
        if self.virtual_relvars.contains_key(relation_name) {
            return Err(DatabaseError::CannotModifyVirtualRelvar(
                relation_name.to_string(),
            ));
        }

        // Get relation metadata
        let metadata = self.engine.get_relation_metadata(relation_name)?;

        // Check tuple type matches
        if !tuple.conforms_to(metadata.relation_type.tuple_type()) {
            return Err(DatabaseError::TupleMismatch);
        }

        // Check type constraints
        if let Some(attr_constraints) = self.type_constraints.get(relation_name) {
            for (attr_name, constraints) in attr_constraints {
                if let Some(value) = tuple.get(attr_name)
                    && !constraints
                        .is_satisfied_by(value)
                        .map_err(|e| DatabaseError::TypeConstraintViolation(e.to_string()))?
                {
                    return Err(DatabaseError::TypeConstraintViolation(format!(
                        "Attribute {} violates constraint",
                        attr_name
                    )));
                }
            }
        }

        // Load current relation to check key constraints
        let current_relation = self.query(relation_name)?;

        // Check key constraints
        if let Some(key_constraints) = self.key_constraints.get(relation_name) {
            if let Some(pk) = key_constraints.primary_key()
                && pk
                    .would_violate(&current_relation, &tuple)
                    .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
            {
                return Err(DatabaseError::PrimaryKeyViolation);
            }

            for ck in key_constraints.candidate_keys() {
                if ck
                    .would_violate(&current_relation, &tuple)
                    .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
                {
                    return Err(DatabaseError::CandidateKeyViolation);
                }
            }
        }

        // Check foreign key constraints
        let fk_constraints_opt = self.foreign_key_constraints.get(relation_name).cloned();
        if let Some(fk_constraints) = fk_constraints_opt {
            for fk in fk_constraints.foreign_keys() {
                let referenced_relation = self.query(fk.referenced_relation_name())?;

                if fk
                    .would_violate_on_insert(&tuple, &referenced_relation)
                    .map_err(|e| DatabaseError::ForeignKeyViolation(e.to_string()))?
                {
                    return Err(DatabaseError::ForeignKeyViolation(
                        "Foreign key constraint violated".to_string(),
                    ));
                }
            }
        }

        // Insert into engine
        self.engine.insert_tuple(relation_name, tuple)?;
        Ok(())
    }

    /// Query a relation (base or virtual).
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationNotFound` if the relation doesn't exist.
    pub fn query(&mut self, relation_name: &str) -> Result<Relation, DatabaseError> {
        // Check if this is a virtual relvar
        if let Some(virtual_relvar) = self.virtual_relvars.get(relation_name) {
            return (virtual_relvar.evaluator)(self);
        }

        // Otherwise, load from engine
        Ok(self.engine.load_relation(relation_name)?)
    }

    /// Delete tuples matching a predicate.
    ///
    /// Returns the number of tuples deleted.
    pub fn delete<F>(&mut self, relation_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
    {
        // Check if trying to delete from a virtual relvar
        if self.virtual_relvars.contains_key(relation_name) {
            return Err(DatabaseError::CannotModifyVirtualRelvar(
                relation_name.to_string(),
            ));
        }

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

        // Check foreign key constraints (other relations referencing this one)
        // TODO: Implement cascading deletes
        let fk_constraints_clone = self.foreign_key_constraints.clone();
        for (ref_name, fk_constraints) in &fk_constraints_clone {
            for fk in fk_constraints.foreign_keys() {
                if fk.referenced_relation_name() == relation_name {
                    let referencing_relation = self.query(ref_name)?;

                    // Check if any referencing tuples would be orphaned
                    for ref_tuple in referencing_relation.tuples() {
                        let ref_key_values: Vec<ScalarValue> = fk
                            .foreign_key_attributes()
                            .iter()
                            .filter_map(|attr| ref_tuple.get(attr).cloned())
                            .collect();

                        // Check if the key exists in the new relation
                        let exists = new_relation.tuples().any(|t| {
                            let key_values: Vec<ScalarValue> = fk
                                .referenced_attributes()
                                .iter()
                                .filter_map(|attr| t.get(attr).cloned())
                                .collect();
                            key_values == ref_key_values
                        });

                        if !exists {
                            return Err(DatabaseError::ForeignKeyViolation(format!(
                                "Deleting tuples would orphan referencing tuples in {}",
                                ref_name
                            )));
                        }
                    }
                }
            }
        }

        // Store the new relation
        self.engine.store_relation(relation_name, &new_relation)?;
        Ok(delete_count)
    }

    /// Update tuples matching a predicate.
    ///
    /// Returns the number of tuples updated.
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
        // Check if trying to update a virtual relvar
        if self.virtual_relvars.contains_key(relation_name) {
            return Err(DatabaseError::CannotModifyVirtualRelvar(
                relation_name.to_string(),
            ));
        }

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

        // Validate key constraints on new relation
        if let Some(key_constraints) = self.key_constraints.get(relation_name) {
            if let Some(pk) = key_constraints.primary_key() {
                for tuple in new_relation.tuples() {
                    if pk
                        .would_violate(&new_relation, tuple)
                        .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
                    {
                        return Err(DatabaseError::PrimaryKeyViolation);
                    }
                }
            }

            for ck in key_constraints.candidate_keys() {
                for tuple in new_relation.tuples() {
                    if ck
                        .would_violate(&new_relation, tuple)
                        .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
                    {
                        return Err(DatabaseError::CandidateKeyViolation);
                    }
                }
            }
        }

        // Store the new relation
        self.engine.store_relation(relation_name, &new_relation)?;
        Ok(update_count)
    }

    /// Begin a transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if a transaction is already in progress.
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
    ///
    /// # Errors
    ///
    /// Returns an error if no transaction is in progress.
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
    ///
    /// # Errors
    ///
    /// Returns an error if no transaction is in progress.
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
    ///
    /// # Errors
    ///
    /// Returns an error if a relvar with this name already exists.
    pub fn define_virtual_relvar(
        &mut self,
        name: &str,
        relation_type: RelationType,
        evaluator: fn(&mut Database<E>) -> Result<Relation, DatabaseError>,
    ) -> Result<(), DatabaseError> {
        if self.relvar_exists(name) {
            return Err(DatabaseError::RelationAlreadyExists(name.to_string()));
        }

        self.virtual_relvars.insert(
            name.to_string(),
            VirtualRelvarDefinition {
                name: name.to_string(),
                relation_type,
                evaluator,
            },
        );

        Ok(())
    }

    /// Drop a virtual relvar.
    ///
    /// # Errors
    ///
    /// Returns an error if the virtual relvar doesn't exist.
    pub fn drop_virtual_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        self.virtual_relvars
            .remove(name)
            .ok_or_else(|| DatabaseError::RelationNotFound(name.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage_engine::InMemoryEngine;
    use crate::tuple;
    use crate::types::{ScalarType, TupleType};

    fn test_rel_type() -> RelationType {
        RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        )
    }

    #[test]
    fn test_create_and_query_relvar() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        db.create_relvar("TEST", test_rel_type()).unwrap();
        assert!(db.relvar_exists("TEST"));

        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        let result = db.query("TEST").unwrap();
        assert_eq!(result.cardinality(), 1);
    }

    #[test]
    fn test_drop_relvar() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        db.create_relvar("TEST", test_rel_type()).unwrap();
        db.drop_relvar("TEST").unwrap();

        assert!(!db.relvar_exists("TEST"));
    }

    #[test]
    fn test_delete() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        db.create_relvar("TEST", test_rel_type()).unwrap();
        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

        let deleted = db
            .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1)
            .unwrap();
        assert_eq!(deleted, 1);

        let result = db.query("TEST").unwrap();
        assert_eq!(result.cardinality(), 1);
    }

    #[test]
    fn test_update() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        db.create_relvar("TEST", test_rel_type()).unwrap();
        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        let updated = db
            .update(
                "TEST",
                |t| t.get_typed::<i64>("id").unwrap() == 1,
                |t| tuple! { id: t.get_typed::<i64>("id").unwrap(), name: "Alicia" },
            )
            .unwrap();
        assert_eq!(updated, 1);

        let result = db.query("TEST").unwrap();
        let tuple = result.tuples().next().unwrap();
        assert_eq!(tuple.get_typed::<String>("name").unwrap(), "Alicia");
    }

    #[test]
    fn test_transactions() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        db.create_relvar("TEST", test_rel_type()).unwrap();
        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        db.begin().unwrap();
        db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

        let result = db.query("TEST").unwrap();
        assert_eq!(result.cardinality(), 2);

        db.rollback().unwrap();

        let result = db.query("TEST").unwrap();
        assert_eq!(result.cardinality(), 1);
    }
}
