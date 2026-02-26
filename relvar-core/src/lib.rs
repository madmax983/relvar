//! # Relvar Core - The Pure Relational Kernel
//!
//! `relvar-core` is the heart of the Relvar database system. It implements the
//! **relational model of data** as strictly defined by C.J. Date and Hugh Darwen
//! in *The Third Manifesto* (TTM).
//!
//! This crate contains the pure logical implementation of:
//! - **Types**: The foundation of the system (Scalar, Tuple, Relation).
//! - **Values**: The data itself (Relations, Tuples, Scalar Values).
//! - **Algebra**: The set of operators for manipulating relations.
//! - **Constraints**: The rules that maintain data integrity.
//! - **Database**: The orchestrator that manages state and transactions.
//!
//! ## Design Philosophy
//!
//! This crate is designed to be **pure** and **storage-agnostic**.
//!
//! - **Zero I/O**: The core logic operates entirely in memory (conceptually).
//!   It doesn't know about files, sockets, or disks.
//! - **Pluggable Storage**: Persistence is handled via the [`StorageEngine`] trait.
//!   Use [`InMemoryEngine`] for testing or [`PersistentEngine`](../relvar_storage/struct.PersistentEngine.html)
//!   (from `relvar-storage`) for disk-based storage.
//! - **Correctness First**: The implementation prioritizes theoretical correctness
//!   over raw performance. It strictly enforces set semantics, strong typing,
//!   and referential integrity.
//!
//! ## Core Concepts
//!
//! ### 1. Types ([`types`])
//! Every value has a type. The type system is the backbone of correctness.
//! - [`ScalarType`]: Atomic types (Int, String, Bool) and **User-Defined Types** (TTM Prescription 1).
//! - [`TupleType`]: A set of named, typed attributes (the "heading").
//! - [`RelationType`]: The type of a relation, defined by its tuple type.
//!
//! ### 2. Values ([`values`])
//! - [`ScalarValue`]: An individual data item (e.g., `42`, `"Hello"`).
//! - [`Tuple`]: A set of named attribute values.
//! - [`Relation`]: A **set** of tuples. No duplicates, no ordering.
//!
//! ### 3. Algebra ([`algebra`])
//! Data is manipulated using relational algebra operators, not SQL.
//! - [`restrict`](values::Relation::restrict): Filter tuples (WHERE).
//! - [`project`](values::Relation::project): Select attributes (SELECT).
//! - [`join`](values::Relation::join): Combine relations (NATURAL JOIN).
//! - [`summarize`](values::Relation::summarize): Aggregate data (GROUP BY).
//!
//! ## Quick Start
//!
//! ```
//! use relvar_core::database::Database;
//! use relvar_core::storage_engine::InMemoryEngine;
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::tuple;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 1. Initialize an in-memory database
//!     let mut db = Database::new(InMemoryEngine::new());
//!
//!     // 2. Define a schema (Relation Type)
//!     let user_type = RelationType::new(
//!         TupleType::new()
//!             .with_attribute("id", ScalarType::Int)
//!             .with_attribute("name", ScalarType::String)
//!             .with_attribute("active", ScalarType::Bool)
//!     );
//!
//!     // 3. Create a Relvar (Relation Variable)
//!     db.create_relvar("USERS", user_type)?;
//!
//!     // 4. Insert data
//!     db.insert("USERS", tuple! {
//!         id: 1i64,
//!         name: "Alice",
//!         active: true
//!     })?;
//!
//!     db.insert("USERS", tuple! {
//!         id: 2i64,
//!         name: "Bob",
//!         active: false
//!     })?;
//!
//!     // 5. Query using Relational Algebra
//!     //    "Get names of active users"
//!     let active_users = db.query("USERS")?
//!         .restrict(|t| t.get_typed::<bool>("active").unwrap_or(false))
//!         .project(&["name"]);
//!
//!     // 6. Verify results
//!     assert_eq!(active_users.cardinality(), 1);
//!     let tuple = active_users.tuples().next().unwrap();
//!     assert_eq!(tuple.get_typed::<String>("name").unwrap(), "Alice");
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]

pub mod algebra;
pub mod constraints;
pub mod database;
pub mod error;
pub mod query;
pub mod storage_engine;
/// Core traits for decoupling components.
pub mod traits;
pub mod types;
pub mod values;

pub mod utils;

pub use database::{Database, DatabaseError};
pub use query::{Query, QueryError};
pub use types::{RelationType, ScalarType, TupleType};
pub use values::{Relation, ScalarValue, Tuple};

// Re-export constraint types
pub use constraints::{
    AttributeConstraints, CandidateKey, CheckConstraint, CheckConstraintError, CheckConstraints,
    CmpOp, ConstraintExpression, ConstraintManagerError, ExpressionError, ForeignKey,
    ForeignKeyConstraints, ForeignKeyError, KeyConstraintError, KeyConstraints, PrimaryKey,
    TypeConstraint, TypeConstraintError, ValueOrRef,
};

// Re-export storage engine types
pub use storage_engine::{InMemoryEngine, StorageEngine, StorageError};

// Re-export core traits
pub use traits::QueryExecutor;
