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

use crate::collections::HashMap;
use crate::constraints::ConstraintManager;
use crate::constraints::assertion::DatabaseAssertion;
use crate::storage_engine::{IsolationLevel, StorageEngine};
use crate::types::{OperatorError, OperatorRegistry, OperatorSignature};
use crate::values::ScalarValue;
use alloc::string::String;
use alloc::vec::Vec;

mod dml;
pub(crate) mod virtual_relvar;

mod assertions;
mod assignment;
mod data;
mod golden_rule;
mod insert_plan;
mod integrity;
mod key_index;
mod schema;
mod statement;
mod transaction;

use self::key_index::KeyIndex;

use self::virtual_relvar::VirtualRelvarDefinition;

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
/// - **DML Operations**: Insert, update, and delete tuples ([`insert`](Database::insert)),
///   or assign several relvars at once ([`multiple_assign`](Database::multiple_assign)).
/// - **Query Execution**: Run relational algebra queries ([`query`](Database::query)).
/// - **Constraint Management**: Enforce keys, foreign keys, and type constraints.
/// - **Transaction Control**: Manage ACID transactions.
///
/// # Examples
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
    pub(crate) engine: E,
    /// Manages all integrity constraints.
    pub(crate) constraints: ConstraintManager,
    /// Whether a transaction is currently in progress.
    pub(crate) in_transaction: bool,
    /// Transaction savepoint.
    pub(crate) transaction_snapshot: Option<E::Snapshot>,
    /// Isolation level of the active transaction, if any.
    ///
    /// Set by [`begin`](Self::begin) /
    /// [`begin_transaction_with_level`](Self::begin_transaction_with_level),
    /// cleared by [`commit`](Self::commit) / [`rollback`](Self::rollback).
    pub(crate) transaction_isolation: Option<IsolationLevel>,
    /// In-memory key index for O(1) key-constraint lookups (#23).
    ///
    /// Maps `(relation name, key attributes)` to the set of key values
    /// present in the relation. Maintained on every write path; see the
    /// [`key_index`](self::key_index) module.
    pub(crate) key_index: KeyIndex,
    /// Snapshot of [`key_index`](Self::key_index) taken at
    /// [`begin`](Self::begin), restored on [`rollback`](Self::rollback).
    pub(crate) key_index_snapshot: Option<KeyIndex>,
    /// Virtual relvars defined by expressions.
    pub(crate) virtual_relvars: HashMap<String, VirtualRelvarDefinition<E>>,
    /// Database assertions (cross-relvar predicates over the whole database state).
    pub(crate) assertions: Vec<DatabaseAssertion<E>>,
    /// `true` while database assertion predicates are being evaluated.
    ///
    /// Predicates must be read-only; DML issued from inside one is
    /// rejected (see the `statement` module).
    pub(crate) evaluating_assertions: bool,
    /// Registry of user-defined scalar operators (TTM RM Prescription 3).
    ///
    /// Operators are resolved by name when expressions containing
    /// [`ScalarExpression::Apply`](crate::constraints::ScalarExpression::Apply)
    /// are evaluated, e.g. in [`Query::restrict`](crate::query::Query::restrict)
    /// predicates executed via [`Query::execute`](crate::query::Query::execute).
    pub(crate) operators: OperatorRegistry,
}

impl<E: StorageEngine> Database<E> {
    /// Creates a new `Database` instance with the specified storage engine.
    ///
    /// By abstracting over the `StorageEngine`, this constructor allows users to create
    /// either purely in-memory databases (for fast testing or temporary data) or
    /// persistent databases (using the `relvar-storage` crate).
    ///
    /// The database's relvars are whatever the engine holds. Its
    /// declarative constraints — keys, foreign keys, CHECK and type
    /// constraints, and declarative assertions
    /// ([`add_declarative_assertion`](Self::add_declarative_assertion)) —
    /// are restored from the engine's stored constraint
    /// catalog ([`StorageEngine::load_constraints`]), so a reopened
    /// persistent database enforces the constraints it was closed with
    /// (TTM RM Prescription 16). A fresh engine, or one that does not
    /// persist constraints, starts with none. Restored constraints are not
    /// re-validated against the stored data; call
    /// [`verify_integrity`](Self::verify_integrity) to re-check a database
    /// that other tools may have written to.
    ///
    /// # Not restored
    ///
    /// Only declarative constraints are persisted. Closure-backed database
    /// assertions ([`add_assertion`](Self::add_assertion)), virtual relvars
    /// ([`define_virtual_relvar`](Self::define_virtual_relvar)) and
    /// user-defined operators ([`register_operator`](Self::register_operator))
    /// are Rust closures or function pointers, which cannot be serialized:
    /// they must be registered again every time the database is opened.
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
    pub fn new(engine: E) -> Self {
        let constraints = engine
            .load_constraints()
            .map_or_else(ConstraintManager::new, ConstraintManager::from_catalog);
        Self {
            engine,
            constraints,
            in_transaction: false,
            transaction_snapshot: None,
            transaction_isolation: None,
            key_index: HashMap::new(),
            key_index_snapshot: None,
            virtual_relvars: HashMap::new(),
            assertions: Vec::new(),
            evaluating_assertions: false,
            operators: OperatorRegistry::new(),
        }
    }

    /// Returns the registry of user-defined scalar operators
    /// (TTM RM Prescription 3).
    ///
    /// The registry is consulted whenever an expression containing an
    /// operator application is evaluated against this database — in
    /// particular by [`Query::execute`](crate::query::Query::execute) for
    /// `restrict` predicates. Register operators with
    /// [`register_operator`](Self::register_operator) or mutate the registry
    /// via [`operator_registry_mut`](Self::operator_registry_mut).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// let db = Database::new(InMemoryEngine::new());
    /// assert!(db.operator_registry().is_empty());
    /// ```
    pub fn operator_registry(&self) -> &OperatorRegistry {
        &self.operators
    }

    /// Returns the registry of user-defined scalar operators, mutably.
    ///
    /// See [`operator_registry`](Self::operator_registry).
    pub fn operator_registry_mut(&mut self) -> &mut OperatorRegistry {
        &mut self.operators
    }

    /// Defines a user-defined scalar operator on this database
    /// (TTM RM Prescription 3).
    ///
    /// This is a convenience wrapper around
    /// [`OperatorRegistry::register`]. The operator becomes usable in
    /// [`ScalarExpression::Apply`](crate::constraints::ScalarExpression)
    /// nodes evaluated against this database (e.g. in query predicates).
    ///
    /// # Errors
    ///
    /// [`OperatorError::InvalidName`] if the signature's name is empty, or
    /// [`OperatorError::AlreadyRegistered`] if this exact signature is
    /// already registered.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{OperatorSignature, ScalarType};
    /// use relvar_core::values::ScalarValue;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// db.register_operator(
    ///     OperatorSignature {
    ///         name: "is_adult".to_string(),
    ///         param_types: vec![ScalarType::Int],
    ///         return_type: ScalarType::Bool,
    ///     },
    ///     |args| match args[0] {
    ///         ScalarValue::Int(age) => Ok(ScalarValue::Bool(age >= 18)),
    ///         _ => unreachable!("signature guarantees an Int argument"),
    ///     },
    /// ).unwrap();
    /// assert!(!db.operator_registry().is_empty());
    /// ```
    pub fn register_operator(
        &mut self,
        signature: OperatorSignature,
        implementation: impl Fn(&[ScalarValue]) -> Result<ScalarValue, OperatorError>
        + Send
        + Sync
        + 'static,
    ) -> Result<(), OperatorError> {
        self.operators.register(signature, implementation)
    }
}

#[cfg(test)]
mod tests;
