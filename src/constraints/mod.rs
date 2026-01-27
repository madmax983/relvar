//! Database constraints for maintaining data integrity.
//!
//! This module implements the constraint system for enforcing data integrity
//! rules on relations. Constraints ensure that the database maintains
//! consistency as data is inserted, updated, or deleted.
//!
//! # Constraint Types
//!
//! - **Key Constraints** - Ensure tuple uniqueness
//!   - `PrimaryKey` - The designated unique identifier for tuples
//!   - `CandidateKey` - Additional unique identifiers
//! - **Referential Integrity** - Ensure relationship validity
//!   - `ForeignKey` - References must point to existing tuples
//! - **Type Constraints** - Ensure value validity
//!   - `TypeConstraint` - Value domain restrictions (range, enum, etc.)
//!
//! # Example
//!
//! ```
//! use relvar::constraints::{PrimaryKey, KeyConstraints, ForeignKey};
//!
//! // Define a primary key on emp_id
//! let pk = PrimaryKey::new(vec!["emp_id".to_string()]).unwrap();
//!
//! // Create key constraints with the primary key
//! let key_constraints = KeyConstraints::new()
//!     .with_primary_key(pk);
//!
//! // Define a foreign key from employees.dept_id to departments.dept_id
//! let fk = ForeignKey::new(
//!     vec!["dept_id".to_string()],
//!     "DEPARTMENTS".to_string(),
//!     vec!["dept_id".to_string()],
//! ).unwrap();
//! ```

pub mod foreign_key;
pub mod key;
pub mod type_constraint;

pub use foreign_key::{ForeignKey, ForeignKeyConstraints, ForeignKeyError};
pub use key::{CandidateKey, KeyConstraintError, KeyConstraints, PrimaryKey};
pub use type_constraint::{AttributeConstraints, TypeConstraint, TypeConstraintError};
