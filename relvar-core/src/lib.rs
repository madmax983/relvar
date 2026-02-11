//! # Relvar Core - Pure TTM Relational Model
//!
//! This crate provides the pure logical relational model implementation
//! with zero I/O dependencies. It includes:
//!
//! - Type system (ScalarType, TupleType, RelationType)
//! - Value system (ScalarValue, Tuple, Relation)
//! - Relational algebra operators
//! - Constraint definitions
//! - StorageEngine trait and InMemoryEngine implementation
//!
//! This crate is 100% in-memory and testable without any file I/O.
//!
//! # Persistence
//!
//! For persistent storage and transaction management, see the `relvar-storage` crate
//! or the main `relvar` crate which integrates both core logic and storage.

#![warn(missing_docs)]

pub mod algebra;
pub mod constraints;
pub mod database;
pub mod error;
pub mod storage_engine;
pub mod types;
pub mod values;

pub use database::{Database, DatabaseError};
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
