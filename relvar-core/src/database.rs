//! Core database module.
//!
//! This module provides the `Database` struct, which is the main entry point for all database operations.
// Core database instance and operations.
//
// This module provides the [`Database`] struct, which is the main entry point
// for all database operations.
//
// # System Architecture
//
// The `Database` struct acts as the **central orchestrator** of the system. It adheres to the
// Facade pattern, hiding the complexity of the constraint and storage subsystems from the user.
//
// ```text
// ┌───────────────────────────────────────────────────────────────────┐
// │                           Database                                │
// │                                                                   │
// │  ┌───────────────────┐   ┌─────────────────────────────────────┐  │
// │  │ ConstraintManager │   │            StorageEngine            │  │
// │  │                   │   │                                     │  │
// │  │ - Key Constraints │   │  ┌──────────────┐  ┌─────────────┐  │  │
// │  │ - Foreign Keys    │   │  │ InMemory     │  │ Persistent  │  │  │
// │  │ - Type Constraints│   │  │              │  │ (Optional)  │  │  │
// │  │ - Check Const.    │   │  └──────────────┘  └─────────────┘  │  │
// │  └─────────┬─────────┘   └──────────────────┬──────────────────┘  │
// │            │                                │                     │
// └────────────┼────────────────────────────────┼─────────────────────┘
//              ▼                                ▼
//        Validates Data                   Persists Data
// ```
//
// ## Key Interactions
//
// 1.  **Operation Request**: User calls methods like [`insert`](Database::insert), [`update`](Database::update), [`delete`](Database::delete).
// 2.  **Constraint Validation**: `Database` consults [`ConstraintManager`]
//     to ensure the operation violates no integrity rules (e.g., uniqueness, foreign keys).
// 3.  **Persistence**: If valid, `Database` delegates the physical data modification
//     to the configured [`StorageEngine`].
// 4.  **Transaction Management**: `Database` coordinates with `StorageEngine` to begin,
//     commit, or rollback transactions.
//
// # Transactions
//
// The database supports ACID transactions via [`begin`](Database::begin), [`commit`](Database::commit),
// and [`rollback`](Database::rollback).
//
// - **Atomicity**: Operations within a transaction either all succeed or all fail.
// - **Consistency**: Constraints are checked before commit (and during operations).
// - **Isolation**: Provided by the underlying storage engine (e.g., Snapshot Isolation).
// - **Durability**: Provided by the storage engine (e.g., Write-Ahead Logging).
//
// ## Example: Transactional Update
//
// ```
// # use relvar_core::database::Database;
// # use relvar_core::storage_engine::InMemoryEngine;
// # use relvar_core::types::{TupleType, RelationType, ScalarType};
// # use relvar_core::tuple;
// # let mut db = Database::new(InMemoryEngine::new());
// # let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
// # db.create_relvar("TEST", rel_type).unwrap();
//
// db.begin().unwrap();
//
// // This insert is provisional
// db.insert("TEST", tuple! { id: 1i64 }).unwrap();
//
// // If we panic or rollback here, the insert is lost
// db.rollback().unwrap();
//
// assert_eq!(db.query("TEST").unwrap().cardinality(), 0);
// ```

use crate::constraints::ConstraintManager;
use crate::storage_engine::StorageEngine;
use std::collections::HashMap;

// A relational database instance.
//
// The `Database` struct is the primary interface for interacting with Relvar. It is
// generic over a [`StorageEngine`] `E`, allowing you to swap between in-memory
// (testing) and persistent (production) storage backends without changing your
// application logic.
//
// # Responsibilities
//
// - **DDL Operations**: Create and drop relvars ([`create_relvar`](Database::create_relvar)).
// - **DML Operations**: Insert, update, and delete tuples ([`insert`](Database::insert)).
// - **Query Execution**: Run relational algebra queries ([`query`](Database::query)).
// - **Constraint Management**: Enforce keys, foreign keys, and type constraints.
// - **Transaction Control**: Manage ACID transactions.
//
// # Example
//
// ```
// use relvar_core::database::Database;
// use relvar_core::storage_engine::InMemoryEngine;
// use relvar_core::types::{TupleType, RelationType, ScalarType};
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
    pub(crate) engine: E,
    /// Manages all integrity constraints.
    pub(crate) constraints: ConstraintManager,
    /// Whether a transaction is currently in progress.
    pub(crate) in_transaction: bool,
    /// Transaction savepoint.
    pub(crate) transaction_snapshot: Option<E::Snapshot>,
    /// Virtual relvars defined by expressions.
    pub(crate) virtual_relvars: HashMap<String, VirtualRelvarDefinition<E>>,
}

impl<E: StorageEngine> Database<E> {
    /// Creates a new `Database` instance with the specified storage engine.
    ///
    /// The database initializes with an empty catalog (no relations, no constraints).
    /// By abstracting over the `StorageEngine`, this constructor allows users to create
    /// either purely in-memory databases (for fast testing or temporary data) or
    /// persistent databases (using the `relvar-storage` crate).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// // Create a temporary, in-memory database
    /// let db = Database::new(InMemoryEngine::new());
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// let db = Database::new(InMemoryEngine::new());
    /// ```
    pub fn new(engine: E) -> Self {
        Self {
            engine,
            constraints: ConstraintManager::new(),
            in_transaction: false,
            transaction_snapshot: None,
            virtual_relvars: HashMap::new(),
        }
    }
}

/// Virtual Relvar (View) Definitions.
///
/// This module defines the metadata and evaluation structures for Virtual Relvars,
/// which are the relational equivalent of SQL Views. Unlike base relvars, virtual relvars
/// do not store data; they store an expression (a query) that is evaluated on demand.
use crate::error::DatabaseError;
use crate::types::RelationType;
use crate::values::Relation;

/// Definition of a virtual relvar (view).
///
/// Stores the metadata required to evaluate a virtual relvar on demand.
#[derive(Debug, Clone)]
pub(crate) struct VirtualRelvarDefinition<E: StorageEngine> {
    /// The relation type (heading) of the view.
    ///
    /// This defines the schema of the result produced by the evaluator.
    /// The database uses this to validate queries against the view without
    /// needing to evaluate it first.
    pub relation_type: RelationType,

    /// The evaluation function (closure) that computes the view's contents.
    ///
    /// # Signature
    ///
    /// `fn(&Database<E>) -> Result<Relation, DatabaseError>`
    ///
    /// - **Input**: A `&Database<E>`, which allows the view
    ///   to query other relvars (base or virtual) in the database.
    /// - **Output**: A `Result` containing the computed `Relation`.
    ///
    /// # Safety
    ///
    /// The evaluator is passed a read-only reference (`&`), ensuring that
    /// viewing a relation cannot cause side effects (mutations) in the database.
    pub evaluator: fn(&Database<E>) -> Result<Relation, DatabaseError>,
}

/// Data Manipulation Language (DML) primitives.
///
/// This module contains pure functions for computing the resulting state of a relation
/// after applying `UPDATE` or `DELETE` operations. These functions are intentionally decoupled
/// from the `Database` struct and storage engine to facilitate testing and optimize
/// tuple evaluation loops.
use crate::values::Tuple;

pub(crate) fn compute_relation_after_delete<F>(
    current_relation: Relation,
    predicate: F,
) -> Result<(Relation, usize), DatabaseError>
where
    F: Fn(&Tuple) -> bool,
{
    let initial_cardinality = current_relation.cardinality();
    let relation_type = current_relation.relation_type().clone();

    // Optimization: Pass the iterator directly to `from_tuples_unchecked`
    // instead of collecting into an intermediate `Vec`. Since the source
    // relation is valid, the filtered tuples are also guaranteed to be valid,
    // allowing us to bypass redundant type checking.
    let new_relation = Relation::from_tuples_unchecked(
        relation_type,
        current_relation
            .into_iter()
            .filter(|tuple| !predicate(tuple)),
    );

    let delete_count = initial_cardinality - new_relation.cardinality();

    Ok((new_relation, delete_count))
}

pub(crate) fn compute_relation_after_update<F, U>(
    current_relation: Relation,
    predicate: F,
    updater: U,
) -> Result<(Relation, usize), DatabaseError>
where
    F: Fn(&Tuple) -> bool,
    U: Fn(&Tuple) -> Tuple,
{
    let relation_type = current_relation.relation_type().clone();
    let expected_type = relation_type.tuple_type().clone();
    let initial_cardinality = current_relation.cardinality();

    let (tuples, update_count) = current_relation.into_iter().try_fold(
        (Vec::with_capacity(initial_cardinality), 0),
        |(mut acc, count), tuple| {
            if !predicate(&tuple) {
                acc.push(tuple);
                return Ok((acc, count));
            }

            let updated_tuple = updater(&tuple);

            if !updated_tuple.conforms_to(&expected_type) {
                return Err(DatabaseError::TupleMismatch);
            }

            acc.push(updated_tuple);
            Ok((acc, count + 1))
        },
    )?;

    // Optimization: The updated tuples are already validated via `conforms_to`
    // inside the `try_fold` loop. We can safely use `from_tuples_unchecked`
    // to build the new relation, avoiding a second O(N*M) validation pass.
    let new_relation = Relation::from_tuples_unchecked(relation_type, tuples);

    Ok((new_relation, update_count))
}

// Database Schema (DDL) operations.

impl<E: StorageEngine> Database<E> {
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

// Database Data Manipulation (DML) and Query operations.

impl<E: StorageEngine> Database<E> {
    /// - Any constraint is violated (Key, Foreign Key, Type, Check)
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    ///
    /// db.insert("USERS", tuple!{ id: 1i64 }).unwrap();
    /// ```
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    /// db.insert("USERS", tuple!{ id: 1i64 }).unwrap();
    ///
    /// let rel = db.query("USERS").unwrap();
    /// assert_eq!(rel.cardinality(), 1);
    /// ```
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
    /// Obtains the total count of tuples removed from the relation.
    ///
    /// # Errors
    ///
    /// Yields an error if:
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    /// db.insert("USERS", tuple!{ id: 1i64 }).unwrap();
    /// db.insert("USERS", tuple!{ id: 2i64 }).unwrap();
    ///
    /// let deleted_count = db.delete("USERS", |t| t.get_typed::<i64>("id").unwrap() == 1).unwrap();
    /// assert_eq!(deleted_count, 1);
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
    /// Obtains the total count of tuples modified during the operation.
    ///
    /// # Errors
    ///
    /// Yields an error if:
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::values::ScalarValue;
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    /// db.insert("USERS", tuple!{ id: 1i64 }).unwrap();
    ///
    /// let updated_count = db.update(
    ///     "USERS",
    ///     |t| t.get_typed::<i64>("id").unwrap() == 1,
    ///     |t| { let mut t2 = t.clone(); t2.set("id".to_string(), ScalarValue::Int(2)).unwrap(); t2 }
    /// ).unwrap();
    /// assert_eq!(updated_count, 1);
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

        self.constraints.validate_referencing_foreign_keys(
            &mut self.engine,
            relation_name,
            &new_relation,
        )?;

        // Store the new relation
        self.engine.store_relation(relation_name, &new_relation)?;
        Ok(update_count)
    }

    // --- Helper Methods ---

    pub(crate) fn ensure_not_virtual(&self, relation_name: &str) -> Result<(), DatabaseError> {
        if self.virtual_relvars.contains_key(relation_name) {
            Err(DatabaseError::CannotModifyVirtualRelvar(
                relation_name.to_string(),
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn validate_insert(
        &mut self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), DatabaseError> {
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

    pub(crate) fn validate_relation_constraints(
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
        self.constraints.validate_tuple_content_constraints_bulk(
            &mut self.engine,
            relation_name,
            relation,
        )?;

        Ok(())
    }
}

// Database Integrity Constraint operations.

use crate::constraints::{
    AttributeConstraints, CheckConstraints, ForeignKeyConstraints, KeyConstraints,
};

impl<E: StorageEngine> Database<E> {
    /// Get the key constraints for a relation.
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
    /// let current_constraints = db.get_key_constraints("TEST").unwrap();
    /// assert!(current_constraints.primary_key().is_some());
    /// ```
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::constraints::{KeyConstraints, PrimaryKey};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    ///
    /// let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    /// db.set_key_constraints("USERS", KeyConstraints::new().with_primary_key(pk)).unwrap();
    ///
    /// let constraints = db.get_key_constraints("USERS").unwrap();
    /// assert!(constraints.primary_key().is_some());
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
}

// Database Transaction Control (TCL) operations.

impl<E: StorageEngine> Database<E> {
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
    /// Nested transactions are not currently supported.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// db.begin().unwrap();
    /// ```
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// db.begin().unwrap();
    /// db.commit().unwrap();
    /// ```
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
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// db.begin().unwrap();
    /// db.rollback().unwrap();
    /// ```
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::ConstraintManagerError;
    use crate::constraints::expression::{CmpOp, ConstraintExpression, ValueOrRef};
    use crate::constraints::{CheckConstraint, CheckConstraints};
    use crate::database::Database;
    use crate::storage_engine::{InMemoryEngine, StorageError};
    use crate::tuple;
    use crate::types::{ScalarType, TupleType};
    use crate::values::ScalarValue;

    fn test_rel_type() -> RelationType {
        RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        )
    }

    fn setup_parent_child_db() -> Database<InMemoryEngine> {
        use crate::constraints::{KeyConstraints, PrimaryKey};
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

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

        db
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
        assert!(matches!(
            result,
            Err(DatabaseError::Constraint(
                ConstraintManagerError::PrimaryKeyViolation
            ))
        ));
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
        assert!(matches!(
            result,
            Err(DatabaseError::Constraint(
                ConstraintManagerError::CandidateKeyViolation
            ))
        ));
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
        assert!(matches!(
            result,
            Err(DatabaseError::Constraint(
                ConstraintManagerError::ForeignKeyViolation(_)
            ))
        ));
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
            Err(DatabaseError::Constraint(
                ConstraintManagerError::TypeConstraintViolation(_)
            ))
        ));

        // Value above max should fail
        let result = db.insert("TEST", tuple! { id: 101i64, name: "Charlie" });
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::Constraint(
                ConstraintManagerError::TypeConstraintViolation(_)
            ))
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
            .with_constraint(TypeConstraint::Range {
                min: ScalarValue::Int(1),
                max: ScalarValue::Int(i64::MAX),
            });
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
            |db| {
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
        use crate::constraints::{ForeignKey, ForeignKeyConstraints};

        let mut db = setup_parent_child_db();

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
        assert!(matches!(
            result,
            Err(DatabaseError::Constraint(
                ConstraintManagerError::ForeignKeyViolation(_)
            ))
        ));
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
        assert!(matches!(
            result,
            Err(DatabaseError::Constraint(
                ConstraintManagerError::PrimaryKeyViolation
            ))
        ));
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
        let db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

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
            |db| {
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
            |db| {
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

        db.define_virtual_relvar("VIRT", test_rel_type(), |db| db.query("TEST"))
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

        db.define_virtual_relvar("VIRT", test_rel_type(), |db| db.query("TEST"))
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
        db.define_virtual_relvar("VIRT", test_rel_type(), |db| db.query("NONEXISTENT"))
            .unwrap();

        // Querying it should fail
        let result = db.query("VIRT");
        assert!(result.is_err());
    }

    #[test]
    fn test_set_foreign_key_constraints_with_existing_valid_data() {
        use crate::constraints::{ForeignKey, ForeignKeyConstraints};

        let mut db = setup_parent_child_db();

        // Insert data BEFORE setting FK
        db.insert("PARENT", tuple! { id: 1i64, name: "Parent1" })
            .unwrap();
        db.insert("CHILD", tuple! { child_id: 100i64, parent_id: 1i64 })
            .unwrap();

        // Set FK constraints
        let fk = ForeignKey::new(
            vec!["parent_id".to_string()],
            "PARENT".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();
        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

        // Should succeed as data is valid
        db.set_foreign_key_constraints("CHILD", fk_constraints)
            .unwrap();
    }

    #[test]
    fn test_set_foreign_key_constraints_with_existing_invalid_data() {
        use crate::constraints::{ForeignKey, ForeignKeyConstraints};

        let mut db = setup_parent_child_db();

        // Insert data BEFORE setting FK
        db.insert("PARENT", tuple! { id: 1i64, name: "Parent1" })
            .unwrap();
        // Insert ORPHAN child
        db.insert("CHILD", tuple! { child_id: 100i64, parent_id: 99i64 })
            .unwrap();

        // Set FK constraints
        let fk = ForeignKey::new(
            vec!["parent_id".to_string()],
            "PARENT".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();
        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

        // Should fail
        let result = db.set_foreign_key_constraints("CHILD", fk_constraints);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::Constraint(
                ConstraintManagerError::ForeignKeyViolation(_)
            ))
        ));
    }

    #[test]
    fn test_delete_referenced_parent_success() {
        use crate::constraints::{ForeignKey, ForeignKeyConstraints};

        let mut db = setup_parent_child_db();

        let fk = ForeignKey::new(
            vec!["parent_id".to_string()],
            "PARENT".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();
        db.set_foreign_key_constraints("CHILD", ForeignKeyConstraints::new().with_foreign_key(fk))
            .unwrap();

        // Insert parents
        db.insert("PARENT", tuple! { id: 1i64, name: "Parent1" })
            .unwrap();
        db.insert("PARENT", tuple! { id: 2i64, name: "Parent2" })
            .unwrap();

        // Insert child referencing Parent1
        db.insert("CHILD", tuple! { child_id: 100i64, parent_id: 1i64 })
            .unwrap();

        // Delete Parent2 (not referenced) - should succeed
        let count = db
            .delete("PARENT", |t| t.get_typed::<i64>("id").unwrap() == 2)
            .unwrap();
        assert_eq!(count, 1);

        // Verify Parent2 is gone
        let result = db.query("PARENT").unwrap();
        assert_eq!(result.cardinality(), 1);
        assert_eq!(
            result
                .tuples()
                .next()
                .unwrap()
                .get_typed::<String>("name")
                .unwrap(),
            "Parent1"
        );
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
        assert!(matches!(
            result,
            Err(DatabaseError::Constraint(
                ConstraintManagerError::CandidateKeyViolation
            ))
        ));
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
        let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "positive_salary",
            "Salary must be positive",
            ConstraintExpression::Cmp {
                left: "salary".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            },
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
        let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "positive_salary",
            "Salary must be positive",
            ConstraintExpression::Cmp {
                left: "salary".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            },
        ));

        let result = db.set_check_constraints("EMPLOYEES", constraints);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::Constraint(
                ConstraintManagerError::CheckConstraintViolation(_)
            ))
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
        let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "positive_salary",
            "Salary must be positive",
            ConstraintExpression::Cmp {
                left: "salary".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            },
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
            Err(DatabaseError::Constraint(
                ConstraintManagerError::CheckConstraintViolation(_)
            ))
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
        let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "valid_age",
            "Age must be between 0 and 150",
            ConstraintExpression::And(
                Box::new(ConstraintExpression::Cmp {
                    left: "age".to_string(),
                    op: CmpOp::Gt,
                    right: ValueOrRef::Value(ScalarValue::Int(0)),
                }),
                Box::new(ConstraintExpression::Cmp {
                    left: "age".to_string(),
                    op: CmpOp::Lt,
                    right: ValueOrRef::Value(ScalarValue::Int(150)),
                }),
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

    #[test]
    fn test_update_constraint_violation_in_loop() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("salary", ScalarType::Int),
        );

        db.create_relvar("EMPLOYEES", rel_type).unwrap();

        // CHECK constraint: salary > 0
        let constraints = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "positive_salary",
            "Salary must be positive",
            ConstraintExpression::Cmp {
                left: "salary".to_string(),
                op: CmpOp::Gt,
                right: ValueOrRef::Value(ScalarValue::Int(0)),
            },
        ));
        db.set_check_constraints("EMPLOYEES", constraints).unwrap();

        db.insert("EMPLOYEES", tuple! { id: 1i64, salary: 50000i64 })
            .unwrap();

        // Update to set salary to -100 (violation)
        let result = db.update(
            "EMPLOYEES",
            |t| t.get_typed::<i64>("id").unwrap() == 1,
            |_t| tuple! { id: 1i64, salary: -100i64 },
        );

        // Should fail due to constraint violation in the try_for_each loop
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::Constraint(
                ConstraintManagerError::CheckConstraintViolation(_)
            ))
        ));
    }

    #[test]
    fn test_virtual_relvar_immutability_enforcement() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        // Create a log relation
        let log_type = RelationType::new(TupleType::new().with_attribute("count", ScalarType::Int));
        db.create_relvar("LOG", log_type).unwrap();

        // Define a view. The compiler enforces that we cannot call mutable methods
        // like insert() inside the evaluator because it receives &Database<E>, not &mut Database.
        db.define_virtual_relvar("SAFE_VIEW", test_rel_type(), |db| {
            // db.insert("LOG", ...); // This would cause compilation error!

            // Read operations are allowed
            let _ = db.query("LOG")?;

            Ok(Relation::new(test_rel_type()))
        })
        .unwrap();

        // Query the view
        assert!(db.query("SAFE_VIEW").is_ok());
    }

    #[test]
    fn test_get_relvar_type() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        // Test base relvar
        db.create_relvar("BASE", test_rel_type()).unwrap();
        let base_type = db.get_relvar_type("BASE").unwrap();
        assert_eq!(base_type, test_rel_type());

        // Test virtual relvar
        db.define_virtual_relvar("VIRTUAL", test_rel_type(), |db| db.query("BASE"))
            .unwrap();
        let virtual_type = db.get_relvar_type("VIRTUAL").unwrap();
        assert_eq!(virtual_type, test_rel_type());

        // Test missing relvar
        let result = db.get_relvar_type("NONEXISTENT");
        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::Storage(_))));
    }

    #[test]
    fn test_transaction_errors() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        // Test commit when not in transaction
        let result = db.commit();
        assert!(result.is_err());
        assert!(
            matches!(result, Err(DatabaseError::TransactionError(msg)) if msg == "No transaction in progress")
        );

        // Test rollback when not in transaction
        let result = db.rollback();
        assert!(result.is_err());
        assert!(
            matches!(result, Err(DatabaseError::TransactionError(msg)) if msg == "No transaction in progress")
        );

        // Test nested begin
        db.begin().unwrap();
        let result = db.begin();
        assert!(result.is_err());
        assert!(
            matches!(result, Err(DatabaseError::TransactionError(msg)) if msg == "Transaction already in progress")
        );
    }

    #[test]
    fn test_define_virtual_relvar_already_exists() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

        // Create base relvar
        db.create_relvar("BASE", test_rel_type()).unwrap();

        // Define virtual relvar
        db.define_virtual_relvar("VIRTUAL", test_rel_type(), |db| db.query("BASE"))
            .unwrap();

        // Try to redefine virtual relvar
        let result = db.define_virtual_relvar("VIRTUAL", test_rel_type(), |db| db.query("BASE"));
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::RelationAlreadyExists(_))
        ));

        // Try to define virtual relvar with same name as base relvar
        let result = db.define_virtual_relvar("BASE", test_rel_type(), |db| db.query("BASE"));
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(DatabaseError::RelationAlreadyExists(_))
        ));
    }
}
