//! Type system for the relational model.
//!
//! This module defines the type hierarchy used throughout Relvar:
//!
//! - `ScalarType` - Primitive and user-defined scalar types
//! - `TupleType` - A heading (set of attribute name-type pairs)
//! - `RelationType` - The type of a relation (defined by its heading)
//!
//! # Type Hierarchy
//!
//! ```text
//! ScalarType          -- Primitive types (Int, Float, String, Bool, Bytes)
//!    │                   or user-defined types (POSSREP pattern)
//!    │                   or relation-valued attributes
//!    v
//! TupleType           -- A set of (attribute_name, ScalarType) pairs
//!    │                   This is the "heading" in relational terminology
//!    v
//! RelationType        -- Wraps a TupleType as the type of a relation
//! ```
//!
//! # TTM Compliance
//!
//! - **Prescription 1**: User-defined scalar types via POSSREP pattern
//! - **Proscription 4**: No attribute ordering (attributes identified by name)
//! - Type equality is structural, not physical
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{ScalarType, TupleType, RelationType};
//!
//! // Define a tuple type (heading)
//! let employee_heading = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String)
//!     .with_attribute("salary", ScalarType::Float);
//!
//! // Create a relation type from the heading
//! let employee_type = RelationType::new(employee_heading);
//!
//! assert_eq!(employee_type.degree(), 3);
//! ```

pub mod relation_type;
pub mod scalar;
pub mod tuple_type;
#[cfg(test)]
mod user_defined_test;

pub use relation_type::RelationType;
pub use scalar::ScalarType;
pub use tuple_type::TupleType;
