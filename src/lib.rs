//! # Relvar - A Pure Relational Database Management System
//!
//! Relvar is a Rust implementation of a relational database management system (RDBMS)
//! that strictly adheres to the principles outlined in *The Third Manifesto* by
//! C.J. Date and Hugh Darwen.
//!
//! ## Core Principles
//!
//! This library implements the relational model with strict adherence to TTM:
//!
//! - **No NULL values** (Proscription 1) - All attributes must have values
//! - **No duplicate tuples** (Proscription 2) - Relations are true sets
//! - **No tuple ordering** (Proscription 3) - Tuples have no inherent order
//! - **No attribute ordering** (Proscription 4) - Attributes are identified by name
//! - **No tuple-level IDs** (Proscription 6) - Physical storage details are never exposed
//!   ```
//!   use relvar::types::{TupleType, RelationType, ScalarType};
//!   use relvar::values::Relation;
//!   use relvar::tuple;
//!
//!   let heading = TupleType::new()
//!       .with_attribute("id", ScalarType::Int)
//!       .with_attribute("name", ScalarType::String);
//!
//!   let mut employees = Relation::new(RelationType::new(heading));
//!
//!   // Insert returns bool (was it new?), not a row ID - tuples are values
//!   let was_new: bool = employees.insert(tuple! { id: 1i64, name: "Alice" }).unwrap();
//!   assert!(was_new);  // No physical address returned
//!
//!   // Query returns tuple values, not physical references
//!   for tuple in employees.tuples() {
//!       // Tuples are accessed by attribute name, never by physical address
//!       let _name = tuple.get("name");
//!   }
//!   ```
//! - **Complete relational algebra** (Prescription 7) - All standard operators implemented
//! - **User-defined types** (Prescription 1) - POSSREP pattern support
//!
//! ## Quick Start
//!
//! ```no_run
//! use relvar::{Database, DatabaseError, ScalarType};
//! use relvar::types::{TupleType, RelationType};
//! use relvar::tuple;
//!
//! // Open or create a database
//! let mut db = Database::open("my_database")?;
//!
//! // Define a relation type (heading)
//! let employee_type = TupleType::new()
//!     .with_attribute("emp_id", ScalarType::Int)
//!     .with_attribute("name", ScalarType::String)
//!     .with_attribute("dept_id", ScalarType::Int);
//!
//! // Create a relation (base relvar)
//! db.create_relvar("EMPLOYEES", RelationType::new(employee_type))?;
//!
//! // Insert tuples
//! db.insert("EMPLOYEES", tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })?;
//! db.insert("EMPLOYEES", tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })?;
//!
//! // Query with relational algebra
//! let result = db.query("EMPLOYEES")?
//!     .restrict(|t| t.get_typed::<i64>("dept_id").unwrap() == 10)
//!     .project(&["emp_id", "name"]);
//!
//! # Ok::<(), DatabaseError>(())
//! ```
//!
//! ## Modules
//!
//! - [`algebra`] - Relational algebra operators (project, restrict, join, union, etc.)
//! - [`constraints`] - Database constraints (primary keys, foreign keys, type constraints)
//! - [`database`] - Core database instance and transaction management
//! - [`storage`] - Persistent storage layer (heap files, B-tree indexes, catalog)
//! - [`types`] - Type system (scalar types, tuple types, relation types)
//! - [`values`] - Runtime values (scalar values, tuples, relations)
//!
//! ## TTM Terminology
//!
//! This library uses terminology from The Third Manifesto:
//!
//! | TTM Term | SQL Equivalent | Description |
//! |----------|----------------|-------------|
//! | Relation | Table | A set of tuples with a common heading |
//! | Tuple | Row | A set of attribute-value pairs |
//! | Attribute | Column | A named component of a tuple |
//! | Heading | Schema | The set of attributes defining a relation's structure |
//! | Relvar | Table variable | A variable whose value is a relation |
//! | Cardinality | Row count | Number of tuples in a relation |
//! | Degree | Column count | Number of attributes in a relation |
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                    Database API                      │
//! │        (create_relvar, insert, query, etc.)         │
//! ├─────────────────────────────────────────────────────┤
//! │                 Relational Algebra                   │
//! │    (project, restrict, join, union, summarize)      │
//! ├─────────────────────────────────────────────────────┤
//! │                   Type System                        │
//! │      (ScalarType, TupleType, RelationType)          │
//! ├─────────────────────────────────────────────────────┤
//! │                  Constraints                         │
//! │    (PrimaryKey, ForeignKey, TypeConstraint)         │
//! ├─────────────────────────────────────────────────────┤
//! │                 Storage Layer                        │
//! │      (HeapFile, BTreeIndex, Catalog, Page)          │
//! └─────────────────────────────────────────────────────┘
//! ```

/// Relational algebra operators for querying and transforming relations.
///
/// This module provides the complete set of relational algebra operators
/// as defined in The Third Manifesto (RM Prescription 7):
///
/// - **Project** (`project`) - Select a subset of attributes
/// - **Restrict** (`restrict`) - Filter tuples by a predicate (σ)
/// - **Rename** (`rename`) - Rename attributes
/// - **Join** (`join`, `theta_join`) - Combine relations
/// - **Union** (`union`) - Set union of type-compatible relations
/// - **Intersect** (`intersect`) - Set intersection
/// - **Difference** (`difference`) - Set difference
/// - **Extend** (`extend`) - Add computed attributes
/// - **Group** (`group`) - Create relation-valued attributes
/// - **Summarize** (`summarize`) - Aggregate with grouping
pub mod algebra;

/// Database constraints for maintaining data integrity.
///
/// This module implements the constraint system per TTM principles:
///
/// - [`KeyConstraints`](constraints::KeyConstraints) - Primary and candidate keys
/// - [`ForeignKey`](constraints::ForeignKey) - Referential integrity
/// - [`TypeConstraint`](constraints::TypeConstraint) - Value domain restrictions
pub mod constraints;

/// Core database instance and operations.
///
/// The [`Database`] struct is the main entry point for:
///
/// - Creating and dropping relations (base relvars)
/// - Inserting, updating, and deleting tuples
/// - Querying relations with relational algebra
/// - Managing transactions (begin, commit, rollback)
/// - Setting up constraints
pub mod database;

/// Persistent storage layer.
///
/// This module provides the physical storage implementation:
///
/// - [`HeapFile`](storage::HeapFile) - Unordered tuple storage
/// - [`BTreeIndex`](storage::BTreeIndex) - Efficient key lookups
/// - [`Catalog`](storage::Catalog) - Relation metadata storage
/// - [`Page`](storage::Page) - Fixed-size disk blocks
///
/// **TTM Compliance:** Physical storage details (e.g., TupleId) are never
/// exposed in public APIs per Proscription 6.
pub mod storage;

/// Type system for relations, tuples, and scalar values.
///
/// This module defines the type hierarchy:
///
/// - [`ScalarType`] - Primitive and user-defined types
/// - [`TupleType`](types::TupleType) - Heading (set of attributes with types)
/// - [`RelationType`](types::RelationType) - Type of a relation
///
/// **TTM Compliance:** User-defined types are supported via the POSSREP
/// (possible representation) pattern per Prescription 1.
pub mod types;

/// Runtime values for relations, tuples, and scalars.
///
/// This module provides the value types:
///
/// - [`ScalarValue`] - Runtime scalar value
/// - [`Tuple`](values::Tuple) - A tuple value conforming to a tuple type
/// - [`Relation`](values::Relation) - A relation value (heading + body)
///
/// **TTM Compliance:** Relations are true sets (no duplicates, no ordering).
/// All values carry their types. No NULL values are permitted.
pub mod values;

pub use database::{Database, DatabaseError};
pub use types::ScalarType;
pub use values::ScalarValue;
