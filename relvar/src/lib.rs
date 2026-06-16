//! # Relvar - A Pure Relational Database Management System
//!
//! Relvar is a Rust implementation of a relational database management system (RDBMS)
//! that strictly adheres to the principles outlined in *The Third Manifesto* (TTM) by
//! C.J. Date and Hugh Darwen.
//!
//! ## Architecture
//!
//! This crate is a facade over:
//! - [`relvar-core`](../relvar_core/index.html) - Pure TTM logical model (always available)
//! - [`relvar-storage`](../relvar_storage/index.html) - Persistent storage (optional, via "storage" feature)
//!
//! ## Examples
//!
//! ### In-memory database (no I/O)
//!
//! ```
//! use relvar::{Database, InMemoryEngine};
//! use relvar::{TupleType, RelationType, ScalarType};
//! use relvar::tuple;
//!
//! let mut db = Database::new(InMemoryEngine::new());
//!
//! let rel_type = RelationType::new(
//!     TupleType::new()
//!         .with_attribute("id", ScalarType::Int)
//!         .with_attribute("name", ScalarType::String)
//! );
//!
//! db.create_relvar("EMPLOYEES", rel_type).unwrap();
//! db.insert("EMPLOYEES", tuple! { id: 1i64, name: "Alice" }).unwrap();
//!
//! let employees = db.query("EMPLOYEES").unwrap();
//! assert_eq!(employees.cardinality(), 1);
//! ```
//!
//! ### Persistent database (requires "storage" feature)
//!
//! ```
//! # #[cfg(feature = "storage")]
//! # {
//! use relvar::{Database, PersistentEngine};
//! use relvar::{TupleType, RelationType, ScalarType};
//! use relvar::tuple;
//! use tempfile::TempDir;
//!
//! // Create a temporary directory for the database
//! let temp_dir = TempDir::new().unwrap();
//! let mut db = Database::new(PersistentEngine::open(temp_dir.path()).unwrap());
//!
//! let rel_type = RelationType::new(
//!     TupleType::new()
//!         .with_attribute("id", ScalarType::Int)
//!         .with_attribute("name", ScalarType::String)
//! );
//!
//! db.create_relvar("EMPLOYEES", rel_type).unwrap();
//! db.insert("EMPLOYEES", tuple! { id: 1i64, name: "Alice" }).unwrap();
//! # }
//! ```
//!
//! ### Advanced Relational Algebra
//!
//! Advanced operators like `extend`, `summarize`, and `group` are available directly on the `Relation` type.
//!
//! ```
//! use relvar::{Database, InMemoryEngine, tuple};
//! use relvar::{TupleType, RelationType, ScalarType};
//! use relvar::ScalarValue;
//! use relvar::algebra::Aggregation;
//!
//! let mut db = Database::new(InMemoryEngine::new());
//! // ... setup relvar ...
//! # let rel_type = RelationType::new(
//! #     TupleType::new()
//! #         .with_attribute("id", ScalarType::Int)
//! #         .with_attribute("salary", ScalarType::Int)
//! # );
//! # db.create_relvar("EMPLOYEES", rel_type).unwrap();
//! # db.insert("EMPLOYEES", tuple! { id: 1i64, salary: 50000i64 }).unwrap();
//!
//! let employees = db.query("EMPLOYEES").unwrap();
//!
//! // Compute annual bonus (10%)
//! let with_bonus = employees.extend("bonus", ScalarType::Int, |t| {
//!     let salary = t.get_typed::<i64>("salary").unwrap();
//!     ScalarValue::Int(salary / 10)
//! }).unwrap();
//!
//! // Summarize total salary
//! let stats = employees.summarize(&[], &[
//!     Aggregation::sum("total_salary", "salary")
//! ]).unwrap();
//! ```
//!
//! ### Transactions
//!
//! Transactions allow grouping multiple operations into an atomic unit.
//!
//! ```
//! use relvar::{Database, InMemoryEngine, tuple};
//! # use relvar::{TupleType, RelationType, ScalarType};
//!
//! let mut db = Database::new(InMemoryEngine::new());
//! # let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
//! # db.create_relvar("TEST", rel_type).unwrap();
//!
//! db.begin().unwrap();
//!
//! // These changes are tentative
//! db.insert("TEST", tuple! { id: 1i64 }).unwrap();
//! db.insert("TEST", tuple! { id: 2i64 }).unwrap();
//!
//! // Commit makes them permanent
//! db.commit().unwrap();
//!
//! // Or use rollback to discard changes
//! db.begin().unwrap();
//! db.insert("TEST", tuple! { id: 3i64 }).unwrap();
//! db.rollback().unwrap();
//! ```

#![warn(missing_docs)]

/// Common types and traits.
pub mod prelude;

// Core Components
pub use relvar_core::DatabaseError;
pub use relvar_core::database::Database;
pub use relvar_core::query::{Query, QueryError};
pub use relvar_core::types::{RelationType, ScalarType, TupleType};
pub use relvar_core::values::{Relation, ScalarValue, Tuple};

// Storage
pub use relvar_core::storage_engine::{InMemoryEngine, StorageEngine, StorageError};

#[cfg(feature = "storage")]
pub use relvar_storage::PersistentEngine;

// Modules
pub use relvar_core::algebra;
pub use relvar_core::constraints;

/// Tuple creation macro.
pub use relvar_core::tuple;

/// Experimental features that may be unstable or subject to change.
pub mod experimental;

/// Developer tools and utilities.
pub mod tools;

#[deprecated(note = "Use `relvar::tools::visualizer` instead")]
pub use tools::visualizer;

/// Data import and export functionality.
#[deprecated(note = "Use `relvar::tools` instead")]
pub(crate) mod data {}

/// Create an in-memory database.
///
/// This is a convenience function for creating a database with an in-memory storage engine.
///
/// # Examples
///
/// ```
/// use relvar;
///
/// let mut db = relvar::in_memory();
/// ```
pub fn in_memory() -> Database<InMemoryEngine> {
    Database::new(InMemoryEngine::new())
}

/// Open or create a persistent database (requires "storage" feature).
///
/// This is a convenience function for creating a database with a persistent storage engine.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "storage")]
/// # {
/// use relvar;
/// use tempfile::TempDir;
///
/// let temp_dir = TempDir::new().unwrap();
/// let mut db = relvar::open(temp_dir.path()).unwrap();
/// # }
/// ```
#[cfg(feature = "storage")]
pub fn open<P: AsRef<std::path::Path>>(
    path: P,
) -> Result<Database<PersistentEngine>, StorageError> {
    Ok(Database::new(PersistentEngine::open(path)?))
}
