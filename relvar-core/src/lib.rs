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

#![warn(missing_docs)]

pub mod algebra;
pub mod constraints;
pub mod database;
pub mod error;
pub mod storage_engine;
pub mod traits;
pub mod types;
pub mod values;
pub mod virtual_relvars;

pub use database::{Database, DatabaseError};
pub use traits::QueryExecutor;
pub use types::{RelationType, ScalarType, TupleType};
pub use values::{Relation, ScalarValue, Tuple};
pub use virtual_relvars::{VirtualRelvarDefinition, VirtualRelvarEvaluator};

// Re-export constraint types
pub use constraints::{
    AttributeConstraints, CandidateKey, CheckConstraint, CheckConstraintError, CheckConstraints,
    CmpOp, ConstraintExpression, ConstraintManagerError, ExpressionError, ForeignKey,
    ForeignKeyConstraints, ForeignKeyError, KeyConstraintError, KeyConstraints, PrimaryKey,
    TypeConstraint, TypeConstraintError, ValueOrRef,
};

// Re-export storage engine types
pub use storage_engine::{InMemoryEngine, StorageEngine, StorageError};

#[cfg(test)]
mod coverage_tests;
