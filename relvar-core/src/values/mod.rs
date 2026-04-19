//! Runtime values for the relational model.
//!
//! This module defines the value types used at runtime:
//!
//! - `ScalarValue` - An atomic value (Int, Float, String, Bool, Bytes, or user-defined)
//! - `Tuple` - A set of attribute-value pairs conforming to a tuple type
//! - `Relation` - A set of tuples conforming to a relation type
//!
//! # Value Hierarchy
//!
//! ```text
//! ScalarValue          -- Atomic values with their types
//!    │
//!    v
//! Tuple               -- Set of (attribute_name, ScalarValue) pairs
//!    │                   Each tuple conforms to a TupleType
//!    v
//! Relation            -- Set of Tuples (no duplicates)
//!                        All tuples conform to the same RelationType
//! ```
//!
//! # TTM Compliance
//!
//! - **Proscription 1**: No NULL values (all attributes must have values)
//! - **Proscription 2**: No duplicate tuples (relations are true sets)
//! - **Proscription 3**: No tuple ordering (tuples are unordered in relations)
//! - **Prescription 1**: User-defined types supported via POSSREP pattern
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::{ScalarValue, Tuple, Relation};
//! use relvar_core::tuple;
//!
//! // Create scalar values
//! let id = ScalarValue::Int(42);
//! let name = ScalarValue::String("Alice".to_string());
//!
//! // Create a tuple using the macro
//! let employee = tuple! {
//!     emp_id: 1i64,
//!     name: "Alice",
//!     active: true,
//! };
//!
//! // Create a relation
//! let heading = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String);
//!
//! let mut employees = Relation::new(RelationType::new(heading));
//! employees.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
//! employees.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
//!
//! assert_eq!(employees.cardinality(), 2);
//! ```

pub(crate) mod relation;
pub(crate) mod scalar;
pub(crate) mod tuple;

pub use relation::Relation;
pub use relation::RelationError;
pub use scalar::ScalarValue;
pub use tuple::Tuple;
pub use tuple::TupleError;
pub use scalar::ScalarValueError;
