//! Core database instance and operations.
//!
//! This module provides the [`Database`] struct, which is the main entry point
//! for all database operations.
//!
//! # System Architecture
//!
//! The `Database` struct acts as the **central orchestrator** of the system. It adheres to the
//! Facade pattern, hiding the complexity of the constraint and storage subsystems from the user.
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────────────┐
//! │                           Database                                │
//! │                                                                   │
//! │  ┌───────────────────┐   ┌─────────────────────────────────────┐  │
//! │  │ ConstraintManager │   │            StorageEngine            │  │
//! │  │                   │   │                                     │  │
//! │  │ - Key Constraints │   │  ┌──────────────┐  ┌─────────────┐  │  │
//! │  │ - Foreign Keys    │   │  │ InMemory     │  │ Persistent  │  │  │
//! │  │ - Type Constraints│   │  │              │  │ (Optional)  │  │  │
//! │  │ - Check Const.    │   │  └──────────────┘  └─────────────┘  │  │
//! │  └─────────┬─────────┘   └──────────────────┬──────────────────┘  │
//! │            │                                │                     │
//! └────────────┼────────────────────────────────┼─────────────────────┘
//!              ▼                                ▼
//!        Validates Data                   Persists Data
//! ```
//!
//! ## Key Interactions
//!
//! 1.  **Operation Request**: User calls methods like [`insert`](Database::insert), [`update`](Database::update), [`delete`](Database::delete).
//! 2.  **Constraint Validation**: `Database` consults [`ConstraintManager`]
//!     to ensure the operation violates no integrity rules (e.g., uniqueness, foreign keys).
//! 3.  **Persistence**: If valid, `Database` delegates the physical data modification
//!     to the configured [`StorageEngine`].
//! 4.  **Transaction Management**: `Database` coordinates with `StorageEngine` to begin,
//!     commit, or rollback transactions.
//!
//! # Transactions
//!
//! The database supports ACID transactions via [`begin`](Database::begin), [`commit`](Database::commit),
//! and [`rollback`](Database::rollback).
//!
//! - **Atomicity**: Operations within a transaction either all succeed or all fail.
//! - **Consistency**: Constraints are checked before commit (and during operations).
//! - **Isolation**: Provided by the underlying storage engine (e.g., Snapshot Isolation).
//! - **Durability**: Provided by the storage engine (e.g., Write-Ahead Logging).
//!
//! ## Example: Transactional Update
//!
//! ```
//! # use relvar_core::database::Database;
//! # use relvar_core::storage_engine::InMemoryEngine;
//! # use relvar_core::types::{TupleType, RelationType, ScalarType};
//! # use relvar_core::tuple;
//! # let mut db = Database::new(InMemoryEngine::new());
//! # let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
//! # db.create_relvar("TEST", rel_type).unwrap();
//!
//! db.begin().unwrap();
//!
//! // This insert is provisional
//! db.insert("TEST", tuple! { id: 1i64 }).unwrap();
//!
//! // If we panic or rollback here, the insert is lost
//! db.rollback().unwrap();
//!
//! assert_eq!(db.query("TEST").unwrap().cardinality(), 0);
//! ```

use crate::constraints::{
    AttributeConstraints, CheckConstraints, ConstraintManager, ForeignKeyConstraints,
    KeyConstraints,
};
pub use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;

use crate::types::RelationType;
use crate::values::{Relation, Tuple};

use std::collections::HashMap;

mod dml;
mod virtual_relvar;

use self::dml::{compute_relation_after_delete, compute_relation_after_update};
pub(crate) use self::virtual_relvar::VirtualRelvarDefinition;

/// A relational database instance.
///
/// The `Database` struct is the primary interface for interacting with Relvar. It is
/// generic over a [`StorageEngine`] `E`, allowing you to swap between in-memory
/// (testing) and persistent (production) storage backends without changing your
/// application logic.
///
/// # Responsibilities
///
/// - **DDL Operations**: Create and drop relvars ([`create_relvar`](Database::create_relvar)).
/// - **DML Operations**: Insert, update, and delete tuples ([`insert`](Database::insert)).
/// - **Query Execution**: Run relational algebra queries ([`query`](Database::query)).
/// - **Constraint Management**: Enforce keys, foreign keys, and type constraints.
/// - **Transaction Control**: Manage ACID transactions.
///
/// # Example
///
/// ```
/// use relvar_core::database::Database;
/// use relvar_core::storage_engine::InMemoryEngine;
/// use relvar_core::types::{TupleType, RelationType, ScalarType};
/// use relvar_core::tuple;
///
/// // 1. Create a database with an in-memory engine
/// let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
///
/// // 2. Define a relation type (schema)
/// let rel_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("id", ScalarType::Int)
///         .with_attribute("name", ScalarType::String)
/// );
///
/// // 3. Create a relation variable (table)
/// db.create_relvar("EMPLOYEES", rel_type).unwrap();
///
/// // 4. Insert data
/// db.insert("EMPLOYEES", tuple! { id: 1i64, name: "Alice" }).unwrap();
///
/// // 5. Query data
/// let employees = db.query("EMPLOYEES").unwrap();
/// assert_eq!(employees.cardinality(), 1);
/// ```
pub struct Database<E: StorageEngine> {
    /// Storage engine for persisting relations.
    engine: E,
    /// Manages all integrity constraints.
    constraints: ConstraintManager,
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
            constraints: ConstraintManager::new(),
            in_transaction: false,
            transaction_snapshot: None,
            virtual_relvars: HashMap::new(),
        }
    }

    /// Create a new base relvar (stored relation).
    ///
    /// # Example
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
    /// Removes the relation variable and all its associated constraints
    /// from the database. This effectively deletes the table and all its data.
    ///
    /// # Example
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
    pub fn drop_relvar(&mut self, name: &str) -> Result<(), DatabaseError> {
        // Remove associated constraints
        self.constraints.remove_constraints_for_relation(name);

        // Drop from engine
        self.engine.drop_relation(name)?;
        Ok(())
    }

    /// Check if a relvar exists (base or virtual).
    ///
    /// # Example
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
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    ///
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
    pub fn get_relvar_type(&self, name: &str) -> Result<RelationType, DatabaseError> {
        // Check virtual relvars first
        if let Some(def) = self.virtual_relvars.get(name) {
            return Ok(def.relation_type.clone());
        }

        // Check base relvars
        let metadata = self.engine.get_relation_metadata(name)?;
        Ok(metadata.relation_type)
    }

    /// Get the key constraints for a relation.
    ///
    /// Retrieving constraints is necessary when dynamically generating data entry
    /// forms or building query optimizers that rely on uniqueness guarantees.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{KeyConstraints, PrimaryKey};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    /// db.create_relvar("TEST", rel_type).unwrap();
    ///
    /// let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    /// let constraints = KeyConstraints::new().with_primary_key(pk);
    /// db.set_key_constraints("TEST", constraints).unwrap();
    ///
    /// let current_constraints = db.get_key_constraints("TEST").unwrap();
    /// assert!(current_constraints.primary_key().is_some());
    /// ```
    pub fn get_key_constraints(&self, relation_name: &str) -> Option<&KeyConstraints> {
        self.constraints.get_key_constraints(relation_name)
    }

    /// Get the foreign key constraints for a relation.
    ///
    /// Retrieving foreign keys is primarily used by the schema visualizer
    /// to draw relationships between relvars, or by automated testing tools
    /// to understand dependency insertion order.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{ForeignKeyConstraints, ForeignKey};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let type1 = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    /// let type2 = RelationType::new(TupleType::new().with_attribute("ref_id", ScalarType::Int));
    /// db.create_relvar("A", type1).unwrap();
    /// db.create_relvar("B", type2).unwrap();
    ///
    /// let fk = ForeignKey::new(
    ///     vec!["ref_id".to_string()],
    ///     "A".to_string(),
    ///     vec!["id".to_string()]
    /// ).unwrap();
    /// let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    /// db.set_foreign_key_constraints("B", constraints).unwrap();
    ///
    /// let current_fks = db.get_foreign_key_constraints("B").unwrap();
    /// assert_eq!(current_fks.foreign_keys().len(), 1);
    /// ```
    pub fn get_foreign_key_constraints(
        &self,
        relation_name: &str,
    ) -> Option<&ForeignKeyConstraints> {
        self.constraints.get_foreign_key_constraints(relation_name)
    }

    /// Set key constraints for a relation.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{KeyConstraints, PrimaryKey};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::Int)
    /// );
    /// db.create_relvar("TEST", rel_type).unwrap();
    ///
    /// let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    /// let constraints = KeyConstraints::new().with_primary_key(pk);
    ///
    /// db.set_key_constraints("TEST", constraints).unwrap();
    /// ```
    pub fn set_key_constraints(
        &mut self,
        relation_name: &str,
        constraints: KeyConstraints,
    ) -> Result<(), DatabaseError> {
        Ok(self
            .constraints
            .set_key_constraints(&mut self.engine, relation_name, constraints)?)
    }

    /// Set foreign key constraints for a relation.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{ForeignKeyConstraints, ForeignKey};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    ///
    /// // Create referenced relvar
    /// let dept_type = RelationType::new(
    ///     TupleType::new().with_attribute("dept_id", ScalarType::Int)
    /// );
    /// db.create_relvar("DEPT", dept_type).unwrap();
    ///
    /// // Create referencing relvar
    /// let emp_type = RelationType::new(
    ///     TupleType::new().with_attribute("dept_id", ScalarType::Int)
    /// );
    /// db.create_relvar("EMP", emp_type).unwrap();
    ///
    /// let fk = ForeignKey::new(
    ///     vec!["dept_id".to_string()],
    ///     "DEPT".to_string(),
    ///     vec!["dept_id".to_string()]
    /// ).unwrap();
    /// let constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
    ///
    /// db.set_foreign_key_constraints("EMP", constraints).unwrap();
    /// ```
    pub fn set_foreign_key_constraints(
        &mut self,
        relation_name: &str,
        constraints: ForeignKeyConstraints,
    ) -> Result<(), DatabaseError> {
        Ok(self.constraints.set_foreign_key_constraints(
            &mut self.engine,
            relation_name,
            constraints,
        )?)
    }

    /// Set type constraints for an attribute.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::AttributeConstraints;
    /// use relvar_core::constraints::TypeConstraint;
    /// use relvar_core::values::ScalarValue;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("count", ScalarType::Int)
    /// );
    /// db.create_relvar("TEST", rel_type).unwrap();
    ///
    /// let attr_constraints = AttributeConstraints::new("count".to_string(), ScalarType::Int)
    ///     .with_constraint(TypeConstraint::Range { min: ScalarValue::Int(1), max: ScalarValue::Int(i64::MAX) });
    ///
    /// db.set_type_constraints("TEST", "count", attr_constraints).unwrap();
    /// ```
    pub fn set_type_constraints(
        &mut self,
        relation_name: &str,
        attribute_name: &str,
        constraints: AttributeConstraints,
    ) -> Result<(), DatabaseError> {
        Ok(self.constraints.set_type_constraints(
            &mut self.engine,
            relation_name,
            attribute_name,
            constraints,
        )?)
    }

    /// Set CHECK constraints for a relation.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::constraints::{CheckConstraints, CheckConstraint, ConstraintExpression, CmpOp, ValueOrRef};
    /// use relvar_core::values::ScalarValue;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("age", ScalarType::Int)
    /// );
    /// db.create_relvar("PEOPLE", rel_type).unwrap();
    ///
    /// // Age must be >= 0
    /// let constraints = CheckConstraints::new()
    ///     .with_constraint(CheckConstraint::new(
    ///         "valid_age",
    ///         "Age must be non-negative",
    ///         ConstraintExpression::Cmp {
    ///             left: "age".to_string(),
    ///             op: CmpOp::Gt,
    ///             right: ValueOrRef::Value(ScalarValue::Int(-1))
    ///         }
    ///     ));
    ///
    /// db.set_check_constraints("PEOPLE", constraints).unwrap();
    /// ```
    pub fn set_check_constraints(
        &mut self,
        relation_name: &str,
        constraints: CheckConstraints,
    ) -> Result<(), DatabaseError> {
        Ok(self
            .constraints
            .set_check_constraints(&mut self.engine, relation_name, constraints)?)
    }

    /// Insert a tuple into a relation.
    ///
    /// This operation validates all constraints (Types, Keys, Foreign Keys, CHECKs)
    /// before modifying the database state.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::Int)
    ///         .with_attribute("name", ScalarType::String)
    /// );
    /// db.create_relvar("USERS", rel_type).unwrap();
    ///
    /// // Successful insert
    /// db.insert("USERS", tuple! { id: 1i64, name: "Alice" }).unwrap();
    ///
    /// // Fails: Tuple type mismatch (missing 'name')
    /// let result = db.insert("USERS", tuple! { id: 2i64 });
    /// assert!(result.is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The relation doesn't exist ([`DatabaseError::RelationNotFound`])
    /// - The tuple type doesn't match the relation's heading ([`DatabaseError::TupleMismatch`])
    /// - Any constraint is violated (Key, Foreign Key, Type, Check)
    pub fn insert(&mut self, relation_name: &str, tuple: Tuple) -> Result<(), DatabaseError> {
        self.ensure_not_virtual(relation_name)?;
        self.validate_insert(relation_name, &tuple)?;

        // Insert into engine
        self.engine.insert_tuple(relation_name, tuple)?;
        Ok(())
    }

    /// Query a relation (base or virtual).
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::Int)
    /// );
    /// db.create_relvar("TEST", rel_type).unwrap();
    /// db.insert("TEST", tuple! { id: 1i64 }).unwrap();
    ///
    /// let result = db.query("TEST").unwrap();
    /// assert_eq!(result.cardinality(), 1);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::RelationNotFound` if the relation doesn't exist.
    pub fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError> {
        // Check if this is a virtual relvar
        if let Some(virtual_relvar) = self.virtual_relvars.get(relation_name) {
            return (virtual_relvar.evaluator)(self);
        }

        // Otherwise, load from engine
        Ok(self.engine.load_relation(relation_name)?)
    }

    /// Delete tuples matching a predicate.
    ///
    /// Retrieves the total count of tuples removed from the relation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The relation doesn't exist ([`DatabaseError::RelationNotFound`])
    /// - Deleting the tuples would violate a foreign key constraint in another relation
    ///   ([`crate::constraints::ConstraintManagerError::ForeignKeyViolation`])
    ///
    /// # Performance
    ///
    /// This operation scans the entire relation to evaluate the predicate.
    /// It then constructs a new relation containing the remaining tuples.
    /// Complexity is O(N) where N is the relation size.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::Int)
    /// );
    /// db.create_relvar("TEST", rel_type).unwrap();
    /// db.insert("TEST", tuple! { id: 1i64 }).unwrap();
    /// db.insert("TEST", tuple! { id: 2i64 }).unwrap();
    ///
    /// // Delete id 1
    /// let deleted_count = db.delete("TEST", |t| {
    ///     t.get_typed::<i64>("id").unwrap() == 1
    /// }).unwrap();
    ///
    /// assert_eq!(deleted_count, 1);
    /// assert_eq!(db.query("TEST").unwrap().cardinality(), 1);
    /// ```
    pub fn delete<F>(&mut self, relation_name: &str, predicate: F) -> Result<usize, DatabaseError>
    where
        F: Fn(&Tuple) -> bool,
    {
        self.ensure_not_virtual(relation_name)?;

        // Load current relation
        let current_relation = self.query(relation_name)?;

        // Filter out tuples to delete
        let (new_relation, delete_count) =
            compute_relation_after_delete(current_relation, predicate)?;

        self.constraints.validate_referencing_foreign_keys(
            &mut self.engine,
            relation_name,
            &new_relation,
        )?;

        // Store the new relation
        self.engine.store_relation(relation_name, &new_relation)?;
        Ok(delete_count)
    }

    /// Update tuples matching a predicate.
    ///
    /// Retrieves the total count of tuples modified during the operation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The relation doesn't exist ([`DatabaseError::RelationNotFound`])
    /// - The updated tuple doesn't match the relation type ([`DatabaseError::TupleMismatch`])
    /// - A key constraint is violated ([`crate::constraints::ConstraintManagerError::PrimaryKeyViolation`], [`crate::constraints::ConstraintManagerError::CandidateKeyViolation`])
    /// - A foreign key constraint is violated ([`crate::constraints::ConstraintManagerError::ForeignKeyViolation`])
    /// - A type constraint is violated ([`crate::constraints::ConstraintManagerError::TypeConstraintViolation`])
    /// - A CHECK constraint is violated ([`crate::constraints::ConstraintManagerError::CheckConstraintViolation`])
    ///
    /// # Performance
    ///
    /// - **Constraint Validation:** Currently, all constraints (CHECK, Type, Foreign Key) are re-validated
    ///   against **every tuple** in the relation after the update, not just the modified ones.
    ///   This ensures total consistency but has O(N) complexity where N is the relation size.
    ///   Future versions may optimize this to O(K) where K is the number of updated tuples.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::Int)
    ///         .with_attribute("salary", ScalarType::Int)
    /// );
    /// db.create_relvar("EMPLOYEES", rel_type).unwrap();
    /// db.insert("EMPLOYEES", tuple! { id: 1i64, salary: 50000i64 }).unwrap();
    ///
    /// // Give a 10% raise to employee 1
    /// let count = db.update(
    ///     "EMPLOYEES",
    ///     |t| t.get_typed::<i64>("id").unwrap() == 1,
    ///     |t| {
    ///         let old_salary = t.get_typed::<i64>("salary").unwrap();
    ///         tuple! {
    ///             id: t.get_typed::<i64>("id").unwrap(),
    ///             salary: old_salary + (old_salary / 10)
    ///         }
    ///     }
    /// ).unwrap();
    ///
    /// assert_eq!(count, 1);
    /// let employees = db.query("EMPLOYEES").unwrap();
    /// let emp1 = employees.tuples().next().unwrap();
    /// assert_eq!(emp1.get_typed::<i64>("salary").unwrap(), 55000);
    /// ```
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
        let (new_relation, update_count) =
            compute_relation_after_update(current_relation, predicate, updater)?;

        self.validate_relation_constraints(relation_name, &new_relation)?;

        // Store the new relation
        self.engine.store_relation(relation_name, &new_relation)?;
        Ok(update_count)
    }

    /// Begins a new transaction.
    ///
    /// This establishes a savepoint (snapshot) of the database. Any changes made
    /// subsequently are provisional until [`commit`](Self::commit) is called.
    ///
    /// # ACID Guarantees
    ///
    /// - **Isolation**: The transaction sees a consistent snapshot of the data.
    /// - **Atomicity**: Changes are not visible to other transactions until commit.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::TransactionError` if a transaction is already active.
    /// Nested transactions are not currently supported.
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

    /// Commits the current transaction.
    ///
    /// Makes all changes since [`begin`](Self::begin) permanent and visible to others.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::TransactionError` if no transaction is in progress.
    ///
    /// # Durability
    ///
    /// If using a persistent storage engine, this ensures all data and WAL entries
    /// are flushed to disk.
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

    /// Rolls back the current transaction.
    ///
    /// Discards all changes made since [`begin`](Self::begin), restoring the database
    /// to its state at the start of the transaction.
    ///
    /// # Errors
    ///
    /// Returns `DatabaseError::TransactionError` if no transaction is in progress.
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
    /// A virtual relvar (or view) acts like a regular relation but is not stored
    /// on disk. Its contents are dynamically generated by evaluating a given function
    /// every time it is queried.
    ///
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::tuple;
    ///
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
    /// // Querying the view
    /// let active_emps = db.query("ACTIVE_EMP").unwrap();
    /// assert_eq!(active_emps.cardinality(), 1);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if a relvar with this name already exists.
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
    /// # Example
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    ///
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

    fn validate_insert(&mut self, relation_name: &str, tuple: &Tuple) -> Result<(), DatabaseError> {
        // Pure checks and Type validations
        self.constraints
            .validate_tuple_type(&self.engine, relation_name, tuple)?;

        self.constraints.validate_tuple_content_constraints(
            &mut self.engine,
            relation_name,
            tuple,
        )?;

        // Load current relation to check key constraints
        let current_relation = self.query(relation_name)?;
        self.constraints.validate_key_constraints_single_tuple(
            relation_name,
            tuple,
            &current_relation,
        )?;

        Ok(())
    }

    fn validate_relation_constraints(
        &mut self,
        relation_name: &str,
        relation: &Relation,
    ) -> Result<(), DatabaseError> {
        // Validate key constraints on new relation
        if let Some(key_constraints) = self.constraints.get_key_constraints(relation_name) {
            self.constraints
                .validate_key_constraints_bulk(relation, key_constraints)?;
        }

        // Validate other constraints (Type, CHECK, FK) on all tuples in the new relation
        // NOTE: In a production system we'd only validate changed tuples, but for now
        // we validate everything to ensure total consistency.
        relation.tuples().try_for_each(|tuple| {
            self.constraints.validate_tuple_content_constraints(
                &mut self.engine,
                relation_name,
                tuple,
            )
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests;
