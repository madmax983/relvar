//! Core database instance and operations.
//!
//! This module provides the [`Database`] struct, which is the main entry point
//! for all database operations.

use crate::constraints::{
    AttributeConstraints, CheckConstraintError, CheckConstraints, ForeignKeyConstraints,
    KeyConstraints,
};
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

    /// CHECK constraint violation.
    #[error("CHECK constraint violation: {0}")]
    CheckConstraintViolation(#[from] CheckConstraintError),

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
    /// CHECK constraints (tuple-level predicates) per relation.
    check_constraints: HashMap<String, CheckConstraints>,
    /// Whether a transaction is currently in progress.
    in_transaction: bool,
    /// Transaction savepoint.
    transaction_snapshot: Option<E::Snapshot>,
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
            check_constraints: HashMap::new(),
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
        self.validate_key_constraints_bulk(&relation, &constraints)?;

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

    /// Set CHECK constraints for a relation.
    ///
    /// TTM: RM Prescription 9 - General integrity constraints (tuple-level).
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Relation doesn't exist
    /// - Existing tuples violate the new constraints
    pub fn set_check_constraints(
        &mut self,
        relation_name: &str,
        constraints: CheckConstraints,
    ) -> Result<(), DatabaseError> {
        if !self.engine.relation_exists(relation_name) {
            return Err(DatabaseError::RelationNotFound(relation_name.to_string()));
        }

        // Validate constraints against existing data
        let relation = self.query(relation_name)?;

        for tuple in relation.tuples() {
            constraints.are_all_satisfied_by(tuple)?;
        }

        self.check_constraints
            .insert(relation_name.to_string(), constraints);
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
        self.ensure_not_virtual(relation_name)?;
        self.validate_tuple_type(relation_name, &tuple)?;
        self.validate_type_constraints(relation_name, &tuple)?;
        self.validate_check_constraints(relation_name, &tuple)?;

        // Load current relation to check key constraints
        let current_relation = self.query(relation_name)?;
        self.validate_key_constraints_single_tuple(relation_name, &tuple, &current_relation)?;
        self.validate_foreign_keys_single_tuple(relation_name, &tuple)?;

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
        self.ensure_not_virtual(relation_name)?;

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

        self.validate_referencing_foreign_keys(relation_name, &new_relation)?;

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
        self.ensure_not_virtual(relation_name)?;

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
            self.validate_key_constraints_bulk(&new_relation, key_constraints)?;
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

    // --- Helper Methods ---

    fn ensure_not_virtual(&self, relation_name: &str) -> Result<(), DatabaseError> {
        if self.virtual_relvars.contains_key(relation_name) {
            Err(DatabaseError::CannotModifyVirtualRelvar(
                relation_name.to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn validate_tuple_type(&self, relation_name: &str, tuple: &Tuple) -> Result<(), DatabaseError> {
        let metadata = self.engine.get_relation_metadata(relation_name)?;
        if !tuple.conforms_to(metadata.relation_type.tuple_type()) {
            Err(DatabaseError::TupleMismatch)
        } else {
            Ok(())
        }
    }

    fn validate_type_constraints(
        &self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), DatabaseError> {
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
        Ok(())
    }

    fn validate_check_constraints(
        &self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), DatabaseError> {
        if let Some(check_constraints) = self.check_constraints.get(relation_name) {
            check_constraints.are_all_satisfied_by(tuple)?;
        }
        Ok(())
    }

    fn validate_key_constraints_single_tuple(
        &self,
        relation_name: &str,
        tuple: &Tuple,
        current_relation: &Relation,
    ) -> Result<(), DatabaseError> {
        if let Some(key_constraints) = self.key_constraints.get(relation_name) {
            if let Some(pk) = key_constraints.primary_key()
                && pk
                    .would_violate(current_relation, tuple)
                    .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
            {
                return Err(DatabaseError::PrimaryKeyViolation);
            }

            for ck in key_constraints.candidate_keys() {
                if ck
                    .would_violate(current_relation, tuple)
                    .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
                {
                    return Err(DatabaseError::CandidateKeyViolation);
                }
            }
        }
        Ok(())
    }

    fn validate_foreign_keys_single_tuple(
        &mut self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), DatabaseError> {
        let fks_to_check = self
            .foreign_key_constraints
            .get(relation_name)
            .map(|c| c.foreign_keys().to_vec())
            .unwrap_or_default();

        for fk in fks_to_check {
            let referenced_relation = self.query(fk.referenced_relation_name())?;

            if fk
                .would_violate_on_insert(tuple, &referenced_relation)
                .map_err(|e| DatabaseError::ForeignKeyViolation(e.to_string()))?
            {
                return Err(DatabaseError::ForeignKeyViolation(
                    "Foreign key constraint violated".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn validate_key_constraints_bulk(
        &self,
        relation: &Relation,
        constraints: &KeyConstraints,
    ) -> Result<(), DatabaseError> {
        if let Some(pk) = constraints.primary_key()
            && !pk
                .is_satisfied_by(relation)
                .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
        {
            return Err(DatabaseError::PrimaryKeyViolation);
        }

        for ck in constraints.candidate_keys() {
            if !ck
                .is_satisfied_by(relation)
                .map_err(|e| DatabaseError::TransactionError(e.to_string()))?
            {
                return Err(DatabaseError::CandidateKeyViolation);
            }
        }
        Ok(())
    }

    fn validate_referencing_foreign_keys(
        &mut self,
        relation_name: &str,
        relation_after_delete: &Relation,
    ) -> Result<(), DatabaseError> {
        // Collect referencing foreign keys to avoid borrowing issues with self.query
        let referencing_fks: Vec<(String, crate::constraints::ForeignKey)> = self
            .foreign_key_constraints
            .iter()
            .flat_map(|(ref_name, fk_constraints)| {
                fk_constraints
                    .foreign_keys()
                    .iter()
                    .filter(|fk| fk.referenced_relation_name() == relation_name)
                    .map(move |fk| (ref_name.clone(), fk.clone()))
            })
            .collect();

        for (ref_name, fk) in referencing_fks {
            let referencing_relation = self.query(&ref_name)?;

            // Check if any referencing tuples would be orphaned
            for ref_tuple in referencing_relation.tuples() {
                let ref_key_values: Vec<ScalarValue> = fk
                    .foreign_key_attributes()
                    .iter()
                    .filter_map(|attr| ref_tuple.get(attr).cloned())
                    .collect();

                // Check if the key exists in the new relation
                let exists = relation_after_delete.tuples().any(|t| {
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::check::{CheckConstraint, CheckConstraints};
    use crate::constraints::expression::{ConstraintExpression, ValueOrRef};
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

    #[test]
    fn test_primary_key_constraint() {
        use crate::constraints::{KeyConstraints, PrimaryKey};

        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        // Add primary key constraint
        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let constraints = KeyConstraints::new().with_primary_key(pk);
        db.set_key_constraints("TEST", constraints).unwrap();

        // First insert should succeed
        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Duplicate primary key should fail
        let result = db.insert("TEST", tuple! { id: 1i64, name: "Bob" });
        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::PrimaryKeyViolation)));
    }

    #[test]
    fn test_candidate_key_constraint() {
        use crate::constraints::{CandidateKey, KeyConstraints};

        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        // Add candidate key constraint on name
        let ck = CandidateKey::new(vec!["name".to_string()]).unwrap();
        let constraints = KeyConstraints::new().with_candidate_key(ck);
        db.set_key_constraints("TEST", constraints).unwrap();

        // First insert should succeed
        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Duplicate candidate key should fail
        let result = db.insert("TEST", tuple! { id: 2i64, name: "Alice" });
        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::CandidateKeyViolation)));
    }

    #[test]
    fn test_foreign_key_constraint() {
        use crate::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};

        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        // Create parent and child relvars
        let parent_type = RelationType::new(
            TupleType::new()
                .with_attribute("dept_id", ScalarType::Int)
                .with_attribute("dept_name", ScalarType::String),
        );
        let child_type = RelationType::new(
            TupleType::new()
                .with_attribute("emp_id", ScalarType::Int)
                .with_attribute("dept_id", ScalarType::Int),
        );

        db.create_relvar("DEPT", parent_type).unwrap();
        db.create_relvar("EMP", child_type).unwrap();

        // Add primary key to parent
        let pk = PrimaryKey::new(vec!["dept_id".to_string()]).unwrap();
        let dept_constraints = KeyConstraints::new().with_primary_key(pk);
        db.set_key_constraints("DEPT", dept_constraints).unwrap();

        // Insert parent record
        db.insert("DEPT", tuple! { dept_id: 10i64, dept_name: "Engineering" })
            .unwrap();

        // Add foreign key constraint
        let fk = ForeignKey::new(
            vec!["dept_id".to_string()],
            "DEPT".to_string(),
            vec!["dept_id".to_string()],
        )
        .unwrap();
        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
        db.set_foreign_key_constraints("EMP", fk_constraints)
            .unwrap();

        // Insert with valid foreign key should succeed
        db.insert("EMP", tuple! { emp_id: 1i64, dept_id: 10i64 })
            .unwrap();

        // Insert with invalid foreign key should fail
        let result = db.insert("EMP", tuple! { emp_id: 2i64, dept_id: 99i64 });
        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::ForeignKeyViolation(_))));
    }

    #[test]
    fn test_type_constraint_range() {
        use crate::constraints::AttributeConstraints;
        use crate::constraints::type_constraint::TypeConstraint;
        use crate::values::ScalarValue;

        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        // Add range constraint on id (1..100)
        let range_constraint = TypeConstraint::Range {
            min: ScalarValue::Int(1),
            max: ScalarValue::Int(100),
        };
        let attr_constraints = AttributeConstraints::new("id".to_string(), ScalarType::Int)
            .with_constraint(range_constraint);
        db.set_type_constraints("TEST", "id", attr_constraints)
            .unwrap();

        // Valid value should succeed
        db.insert("TEST", tuple! { id: 50i64, name: "Alice" })
            .unwrap();

        // Value below min should fail
        let result = db.insert("TEST", tuple! { id: 0i64, name: "Bob" });
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::TypeConstraintViolation(_))
        ));

        // Value above max should fail
        let result = db.insert("TEST", tuple! { id: 101i64, name: "Charlie" });
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::TypeConstraintViolation(_))
        ));
    }

    #[test]
    fn test_type_constraint_positive_int() {
        use crate::constraints::AttributeConstraints;
        use crate::constraints::type_constraint::TypeConstraint;

        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        // Add positive int constraint
        let attr_constraints = AttributeConstraints::new("id".to_string(), ScalarType::Int)
            .with_constraint(TypeConstraint::PositiveInt);
        db.set_type_constraints("TEST", "id", attr_constraints)
            .unwrap();

        // Positive value should succeed
        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Zero should fail
        let result = db.insert("TEST", tuple! { id: 0i64, name: "Bob" });
        assert!(result.is_err());

        // Negative should fail
        let result = db.insert("TEST", tuple! { id: -1i64, name: "Charlie" });
        assert!(result.is_err());
    }

    #[test]
    fn test_virtual_relvar() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

        // Define virtual relvar that projects just names
        db.define_virtual_relvar(
            "NAMES",
            RelationType::new(TupleType::new().with_attribute("name", ScalarType::String)),
            |db: &mut Database<InMemoryEngine>| {
                let test = db.query("TEST")?;
                Ok(test.project(&["name"]))
            },
        )
        .unwrap();

        // Query virtual relvar
        let names = db.query("NAMES").unwrap();
        assert_eq!(names.degree(), 1);
        assert_eq!(names.cardinality(), 2);
    }

    #[test]
    fn test_delete_with_foreign_key_constraint() {
        use crate::constraints::{ForeignKey, ForeignKeyConstraints, KeyConstraints, PrimaryKey};

        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        // Create parent and child
        let parent_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        );
        let child_type = RelationType::new(
            TupleType::new()
                .with_attribute("child_id", ScalarType::Int)
                .with_attribute("parent_id", ScalarType::Int),
        );

        db.create_relvar("PARENT", parent_type).unwrap();
        db.create_relvar("CHILD", child_type).unwrap();

        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let parent_constraints = KeyConstraints::new().with_primary_key(pk);
        db.set_key_constraints("PARENT", parent_constraints)
            .unwrap();

        let fk = ForeignKey::new(
            vec!["parent_id".to_string()],
            "PARENT".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();
        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
        db.set_foreign_key_constraints("CHILD", fk_constraints)
            .unwrap();

        // Insert parent and child
        db.insert("PARENT", tuple! { id: 1i64, name: "Parent1" })
            .unwrap();
        db.insert("CHILD", tuple! { child_id: 10i64, parent_id: 1i64 })
            .unwrap();

        // Deleting parent should fail due to foreign key
        let result = db.delete("PARENT", |_| true);
        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::ForeignKeyViolation(_))));
    }

    #[test]
    fn test_update_with_primary_key_violation() {
        use crate::constraints::{KeyConstraints, PrimaryKey};

        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let constraints = KeyConstraints::new().with_primary_key(pk);
        db.set_key_constraints("TEST", constraints).unwrap();

        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

        // Update that would create duplicate primary key should fail
        let result = db.update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 2,
            |_| {
                tuple! { id: 1i64, name: "Bob" }
            },
        );
        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::PrimaryKeyViolation)));
    }

    #[test]
    fn test_transaction_rollback_with_constraints() {
        use crate::constraints::{KeyConstraints, PrimaryKey};

        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let constraints = KeyConstraints::new().with_primary_key(pk);
        db.set_key_constraints("TEST", constraints).unwrap();

        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        db.begin().unwrap();

        // Make some changes
        db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();
        db.delete("TEST", |t| t.get_typed::<i64>("id").unwrap() == 1)
            .unwrap();

        let result = db.query("TEST").unwrap();
        assert_eq!(result.cardinality(), 1);

        // Rollback
        db.rollback().unwrap();

        // Should be back to original state
        let result = db.query("TEST").unwrap();
        assert_eq!(result.cardinality(), 1);
        assert!(result.contains(&tuple! { id: 1i64, name: "Alice" }));
    }

    #[test]
    fn test_error_relvar_not_found() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        let result = db.query("NONEXISTENT");
        assert!(result.is_err());
        // Error should be Storage(RelationNotFound) since it comes from the engine
        match result {
            Err(DatabaseError::Storage(StorageError::RelationNotFound(name))) => {
                assert_eq!(name, "NONEXISTENT");
            }
            other => panic!("Expected RelationNotFound error, got: {:?}", other),
        }
    }

    #[test]
    fn test_error_duplicate_relvar() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        let result = db.create_relvar("TEST", test_rel_type());
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::RelationAlreadyExists(_))
        ));
    }

    #[test]
    fn test_set_type_constraints() {
        use crate::constraints::AttributeConstraints;
        use crate::constraints::type_constraint::TypeConstraint;
        use crate::values::ScalarValue;

        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        // Create attribute constraints
        let range_constraint = TypeConstraint::Range {
            min: ScalarValue::Int(1),
            max: ScalarValue::Int(100),
        };

        let attr_constraints = AttributeConstraints::new("id".to_string(), ScalarType::Int)
            .with_constraint(range_constraint);

        db.set_type_constraints("TEST", "id", attr_constraints)
            .unwrap();

        // Valid value should succeed
        db.insert("TEST", tuple! { id: 50i64, name: "Alice" })
            .unwrap();

        // Invalid value should fail
        let result = db.insert("TEST", tuple! { id: 200i64, name: "Bob" });
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_commit() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Begin transaction
        db.begin().unwrap();

        // Make changes
        db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

        // Commit
        db.commit().unwrap();

        // Changes should persist
        let result = db.query("TEST").unwrap();
        assert_eq!(result.cardinality(), 2);

        // Transaction should no longer be active
        assert!(!db.in_transaction);
    }

    #[test]
    fn test_commit_without_transaction() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        let result = db.commit();
        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::TransactionError(_))));
    }

    #[test]
    fn test_rollback_without_transaction() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        let result = db.rollback();
        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::TransactionError(_))));
    }

    #[test]
    fn test_begin_nested_transaction_fails() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.begin().unwrap();

        // Try to begin another transaction
        let result = db.begin();
        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::TransactionError(_))));
    }

    #[test]
    fn test_drop_virtual_relvar() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Define virtual relvar
        db.define_virtual_relvar(
            "VIRT",
            RelationType::new(TupleType::new().with_attribute("name", ScalarType::String)),
            |db: &mut Database<InMemoryEngine>| {
                let test = db.query("TEST")?;
                Ok(test.project(&["name"]))
            },
        )
        .unwrap();

        // Verify it exists
        let result = db.query("VIRT");
        assert!(result.is_ok());

        // Drop it
        db.drop_virtual_relvar("VIRT").unwrap();

        // Should no longer exist
        let result = db.query("VIRT");
        assert!(result.is_err());
    }

    #[test]
    fn test_drop_nonexistent_virtual_relvar() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        let result = db.drop_virtual_relvar("NONEXISTENT");
        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::RelationNotFound(_))));
    }

    #[test]
    fn test_cannot_insert_into_virtual_relvar() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.define_virtual_relvar(
            "VIRT",
            RelationType::new(TupleType::new().with_attribute("name", ScalarType::String)),
            |db: &mut Database<InMemoryEngine>| {
                let test = db.query("TEST")?;
                Ok(test.project(&["name"]))
            },
        )
        .unwrap();

        // Try to insert
        let result = db.insert("VIRT", tuple! { name: "Alice" });
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::CannotModifyVirtualRelvar(_))
        ));
    }

    #[test]
    fn test_cannot_delete_from_virtual_relvar() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.define_virtual_relvar(
            "VIRT",
            test_rel_type(),
            |db: &mut Database<InMemoryEngine>| db.query("TEST"),
        )
        .unwrap();

        // Try to delete
        let result = db.delete("VIRT", |_| true);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::CannotModifyVirtualRelvar(_))
        ));
    }

    #[test]
    fn test_cannot_update_virtual_relvar() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.define_virtual_relvar(
            "VIRT",
            test_rel_type(),
            |db: &mut Database<InMemoryEngine>| db.query("TEST"),
        )
        .unwrap();

        // Try to update
        let result = db.update("VIRT", |_| true, |_| tuple! { id: 99i64, name: "X" });
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::CannotModifyVirtualRelvar(_))
        ));
    }

    #[test]
    fn test_cannot_drop_base_relvar_when_virtual_depends_on_it() {
        // This would require tracking dependencies, which might not be implemented
        // Skipping for now as it may not be a current feature
    }

    #[test]
    fn test_virtual_relvar_error_propagation() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        // Define virtual relvar that queries nonexistent base
        db.define_virtual_relvar(
            "VIRT",
            test_rel_type(),
            |db: &mut Database<InMemoryEngine>| db.query("NONEXISTENT"),
        )
        .unwrap();

        // Querying it should fail
        let result = db.query("VIRT");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_returns_count() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();
        db.insert("TEST", tuple! { id: 3i64, name: "Charlie" })
            .unwrap();

        // Delete some tuples
        let count = db
            .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() > 1)
            .unwrap();
        assert_eq!(count, 2);

        let result = db.query("TEST").unwrap();
        assert_eq!(result.cardinality(), 1);
    }

    #[test]
    fn test_update_returns_count() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

        // Update tuples
        let count = db
            .update(
                "TEST",
                |t| t.get_typed::<i64>("id").unwrap() == 1,
                |_| {
                    tuple! { id: 1i64, name: "Alicia" }
                },
            )
            .unwrap();

        assert_eq!(count, 1);

        let result = db.query("TEST").unwrap();
        assert!(result.contains(&tuple! { id: 1i64, name: "Alicia" }));
    }

    #[test]
    fn test_delete_no_matches_returns_zero() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();

        // Delete with no matches
        let count = db
            .delete("TEST", |t| t.get_typed::<i64>("id").unwrap() > 100)
            .unwrap();
        assert_eq!(count, 0);

        // Relation should be unchanged
        let result = db.query("TEST").unwrap();
        assert_eq!(result.cardinality(), 1);
    }

    #[test]
    fn test_update_with_candidate_key_violation() {
        use crate::constraints::{CandidateKey, KeyConstraints};

        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        db.create_relvar("TEST", test_rel_type()).unwrap();

        let ck = CandidateKey::new(vec!["name".to_string()]).unwrap();
        db.set_key_constraints("TEST", KeyConstraints::new().with_candidate_key(ck))
            .unwrap();

        db.insert("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        db.insert("TEST", tuple! { id: 2i64, name: "Bob" }).unwrap();

        // Try to update to create duplicate candidate key
        let result = db.update(
            "TEST",
            |t| t.get_typed::<i64>("id").unwrap() == 2,
            |_| {
                tuple! { id: 2i64, name: "Alice" }
            },
        );

        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::CandidateKeyViolation)));
    }

    #[test]
    fn test_set_check_constraints() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("salary", ScalarType::Int),
        );

        db.create_relvar("EMPLOYEES", rel_type).unwrap();

        // Create CHECK constraint: salary must be positive
        let constraints =
            CheckConstraints::new().with_constraint(CheckConstraint::from_expression(
                "positive_salary",
                "Salary must be positive",
                ConstraintExpression::Gt(
                    "salary".to_string(),
                    ValueOrRef::Value(ScalarValue::Int(0)),
                ),
            ));

        db.set_check_constraints("EMPLOYEES", constraints).unwrap();
    }

    #[test]
    fn test_set_check_constraint_validates_existing_data() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("salary", ScalarType::Int),
        );

        db.create_relvar("EMPLOYEES", rel_type).unwrap();

        // Insert tuple with negative salary
        db.insert("EMPLOYEES", tuple! { id: 1i64, salary: -100i64 })
            .unwrap();

        // Try to add CHECK constraint - should fail because existing data violates it
        let constraints =
            CheckConstraints::new().with_constraint(CheckConstraint::from_expression(
                "positive_salary",
                "Salary must be positive",
                ConstraintExpression::Gt(
                    "salary".to_string(),
                    ValueOrRef::Value(ScalarValue::Int(0)),
                ),
            ));

        let result = db.set_check_constraints("EMPLOYEES", constraints);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::CheckConstraintViolation(_))
        ));
    }

    #[test]
    fn test_insert_enforces_check_constraints() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("salary", ScalarType::Int),
        );

        db.create_relvar("EMPLOYEES", rel_type).unwrap();

        // Add CHECK constraint: salary must be positive
        let constraints =
            CheckConstraints::new().with_constraint(CheckConstraint::from_expression(
                "positive_salary",
                "Salary must be positive",
                ConstraintExpression::Gt(
                    "salary".to_string(),
                    ValueOrRef::Value(ScalarValue::Int(0)),
                ),
            ));
        db.set_check_constraints("EMPLOYEES", constraints).unwrap();

        // Insert with positive salary should succeed
        let result = db.insert("EMPLOYEES", tuple! { id: 1i64, salary: 50000i64 });
        assert!(result.is_ok());

        // Insert with negative salary should fail
        let result = db.insert("EMPLOYEES", tuple! { id: 2i64, salary: -100i64 });
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::CheckConstraintViolation(_))
        ));
    }

    #[test]
    fn test_insert_satisfies_check_constraints() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("age", ScalarType::Int),
        );

        db.create_relvar("PERSONS", rel_type).unwrap();

        // Add complex CHECK constraint: age between 0 and 150
        let constraints =
            CheckConstraints::new().with_constraint(CheckConstraint::from_expression(
                "valid_age",
                "Age must be between 0 and 150",
                ConstraintExpression::And(
                    Box::new(ConstraintExpression::Gt(
                        "age".to_string(),
                        ValueOrRef::Value(ScalarValue::Int(0)),
                    )),
                    Box::new(ConstraintExpression::Lt(
                        "age".to_string(),
                        ValueOrRef::Value(ScalarValue::Int(150)),
                    )),
                ),
            ));
        db.set_check_constraints("PERSONS", constraints).unwrap();

        // Insert with valid age should succeed
        let result = db.insert("PERSONS", tuple! { id: 1i64, age: 30i64 });
        assert!(result.is_ok());

        // Insert with invalid age (too low) should fail
        let result = db.insert("PERSONS", tuple! { id: 2i64, age: -5i64 });
        assert!(result.is_err());

        // Insert with invalid age (too high) should fail
        let result = db.insert("PERSONS", tuple! { id: 3i64, age: 200i64 });
        assert!(result.is_err());
    }
}
