//! Relational algebra operators for querying and transforming relations.
//!
//! This module implements the complete set of relational algebra operators
//! as defined in *The Third Manifesto* (RM Prescription 7). All operators
//! are implemented as methods on the [`Relation`](crate::values::Relation) type.
//!
//! # Why Relational Algebra?
//!
//! Relational algebra is the functional programming language of data.
//! Just as you compose functions like `map`, `filter`, and `fold` to transform collections
//! in Rust, you compose relational operators to transform relations.
//!
//! - **Composable**: The output of every operator is a relation, so you can chain them endlessly.
//! - **Declarative**: You specify *what* result you want, not *how* to compute it.
//! - **Safe**: The type system guarantees that every intermediate result is a valid relation.
//!
//! # Operators
//!
//! ## Set Operators (require type-compatible relations)
//!
//! | Operator | Method | Description |
//! |----------|--------|-------------|
//! | Union | [`union()`](crate::values::Relation::union) | Combines tuples from both relations |
//! | Intersect | [`intersect()`](crate::values::Relation::intersect) | Returns tuples in both relations |
//! | Difference | [`difference()`](crate::values::Relation::difference) | Returns tuples in first but not second |
//!
//! ## Projection and Selection
//!
//! | Operator | Method | Description |
//! |----------|--------|-------------|
//! | Project | [`project()`](crate::values::Relation::project) | Selects a subset of attributes |
//! | Restrict | [`restrict()`](crate::values::Relation::restrict) | Filters tuples by predicate (σ) |
//! | Rename | [`rename()`](crate::values::Relation::rename) | Renames attributes |
//!
//! ## Join Operators
//!
//! | Operator | Method | Description | Tutorial D Alias |
//! |----------|--------|-------------|------------------|
//! | Natural Join | [`join()`](crate::values::Relation::join) | Joins on common attributes | `JOIN` |
//! | Theta Join | [`theta_join()`](crate::values::Relation::theta_join) | Joins with arbitrary predicate | - |
//! | Semijoin | [`semijoin()`](crate::values::Relation::semijoin) | Tuples from A matching B | `MATCHING` |
//! | Semidifference | [`semidifference()`](crate::values::Relation::semidifference) | Tuples from A not matching B | `NOT MATCHING` |
//! | Division | [`divide()`](crate::values::Relation::divide) | Relational division (A ÷ B) | `DIVIDEBY` |
//!
//! ## Extended Operators
//!
//! | Operator | Method | Description |
//! |----------|--------|-------------|
//! | Extend | [`extend()`](crate::values::Relation::extend) | Adds computed attributes |
//! | Group | [`group()`](crate::values::Relation::group) | Creates relation-valued attributes |
//! | Ungroup | [`ungroup()`](crate::values::Relation::ungroup) | Flattens relation-valued attributes |
//! | Summarize | [`summarize()`](crate::values::Relation::summarize) | Aggregation with grouping |
//! | TClose | [`tclose()`](crate::values::Relation::tclose) | Transitive closure of binary relation |
//!
//! # Example
//!
//! ```
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::Relation;
//! use relvar_core::tuple;
//!
//! // Create employee relation
//! let emp_type = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String)
//!     .with_attribute("dept_id", ScalarType::Int);
//!
//! let mut employees = Relation::new(RelationType::new(emp_type));
//! employees.insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();
//! employees.insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 }).unwrap();
//!
//! // Project to just names
//! let names = employees.project(&["name"]);
//! assert_eq!(names.degree(), 1);
//!
//! // Restrict to dept 10
//! let dept10 = employees.restrict(|t| {
//!     t.get_typed::<i64>("dept_id").unwrap() == 10
//! });
//! assert_eq!(dept10.cardinality(), 1);
//! ```
//!
//! # TTM Compliance
//!
//! All operators maintain set semantics:
//! - Duplicate tuples are automatically eliminated
//! - Tuple ordering is not guaranteed
//! - Results are always valid relations

/// Relational Delta operator.
pub mod delta;

/// Set difference operator (A MINUS B).
pub(crate) mod difference;

/// Relational division operator.
pub(crate) mod divide;

/// Extend operator for adding computed attributes.
pub(crate) mod extend;

/// Group and ungroup operators for relation-valued attributes.
pub(crate) mod group;

/// Set intersection operator.
pub(crate) mod intersect;

/// Natural join and theta join operators.
pub(crate) mod join;

/// Project operator for attribute selection (π).
pub(crate) mod project;

/// Rename operator for attribute renaming (ρ).
pub(crate) mod rename;

/// Restrict operator for tuple filtering (σ).
pub(crate) mod restrict;

/// Semijoin and semidifference (antijoin) operators.
pub(crate) mod semijoin;

/// Summarize operator for aggregation with grouping.
pub(crate) mod summarize;
pub use summarize::{Aggregation, AggregationFn};

/// Set union operator.
pub(crate) mod union;

/// Transitive closure operator (TCLOSE).
pub(crate) mod tclose;
