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

use crate::constraints::ConstraintManager;
use crate::storage_engine::StorageEngine;
use std::collections::HashMap;

mod dml;
pub(crate) mod virtual_relvar;

mod data;
mod integrity;
mod schema;
mod transaction;

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
/// - **DML Operations**: Insert, update, and delete tuples ([`insert`](Database::insert)).
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

#[cfg(test)]
mod tests;
