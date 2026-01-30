//! # Relvar - A Pure Relational Database Management System
//!
//! Relvar is a Rust implementation of a relational database management system (RDBMS)
//! that strictly adheres to the principles outlined in *The Third Manifesto* by
//! C.J. Date and Hugh Darwen.
//!
//! ## Architecture
//!
//! This crate is a facade over:
//! - `relvar-core` - Pure TTM logical model (always available)
//! - `relvar-storage` - Persistent storage (optional, via "storage" feature)
//!
//! ## Examples
//!
//! ### In-memory database (no I/O)
//!
//! ```
//! use relvar::{Database, InMemoryEngine};
//! use relvar::types::{TupleType, RelationType, ScalarType};
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
//! ```no_run
//! # #[cfg(feature = "storage")]
//! # {
//! use relvar::{Database, PersistentEngine};
//! use relvar::types::{TupleType, RelationType, ScalarType};
//! use relvar::tuple;
//!
//! let mut db = Database::new(PersistentEngine::open("my_db").unwrap());
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
//! Advanced operators like `extend`, `summarize`, and `group` require importing
//! their corresponding traits. These are re-exported in `relvar::algebra`.
//!
//! ```
//! use relvar::{Database, InMemoryEngine, tuple};
//! use relvar::types::{TupleType, RelationType, ScalarType};
//! use relvar::values::ScalarValue;
//! // Import traits for advanced operators!
//! use relvar::algebra::{ExtendOps, SummarizeOps, Aggregation};
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
//! # use relvar::types::{TupleType, RelationType, ScalarType};
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

// Re-export everything from relvar-core
pub use relvar_core::*;

// Re-export storage components when feature is enabled
#[cfg(feature = "storage")]
pub use relvar_storage::PersistentEngine;

/// Create an in-memory database.
///
/// This is a convenience function for creating a database with an in-memory storage engine.
///
/// # Example
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
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "storage")]
/// # {
/// use relvar;
///
/// let mut db = relvar::open("my_database").unwrap();
/// # }
/// ```
#[cfg(feature = "storage")]
pub fn open<P: AsRef<std::path::Path>>(
    path: P,
) -> Result<Database<PersistentEngine>, StorageError> {
    Ok(Database::new(PersistentEngine::open(path)?))
}
