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
