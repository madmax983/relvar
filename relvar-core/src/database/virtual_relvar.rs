//! Virtual relvar (view) definition and management.
//!
//! A **Virtual Relvar** (known as a "VIEW" in SQL) is a relation variable whose value
//! is defined by a relational expression rather than by stored data.
//!
//! # TTM Compliance
//!
//! This implementation adheres to **RM Prescription 10** from *The Third Manifesto*:
//! > A virtual relvar is a relvar whose value at any given time is the result of
//! > evaluating a certain relational expression at that time.
//!
//! Key properties:
//! - **Lazy Evaluation**: The expression is evaluated *only* when the relvar is queried.
//! - **Consistency**: Because it is re-evaluated on demand, a virtual relvar always
//!   reflects the current state of the base relvars it depends on.
//! - **Read-Only (Currently)**: Virtual relvars in this implementation are read-only.
//!   Updates must be performed on the underlying base relvars.
//!
//! # Example
//!
//! ```
//! use relvar_core::database::Database;
//! use relvar_core::storage_engine::InMemoryEngine;
//! use relvar_core::types::{TupleType, RelationType, ScalarType};
//! use relvar_core::values::Relation;
//! use relvar_core::tuple;
//! use relvar_core::traits::QueryExecutor;
//!
//! // 1. Setup Database
//! let mut db = Database::new(InMemoryEngine::new());
//!
//! // 2. Create Base Relvar "EMPLOYEES"
//! let emp_type = RelationType::new(
//!     TupleType::new()
//!         .with_attribute("id", ScalarType::Int)
//!         .with_attribute("name", ScalarType::String)
//!         .with_attribute("dept_id", ScalarType::Int)
//! );
//! db.create_relvar("EMPLOYEES", emp_type).unwrap();
//!
//! db.insert("EMPLOYEES", tuple! { id: 1i64, name: "Alice", dept_id: 10i64 }).unwrap();
//! db.insert("EMPLOYEES", tuple! { id: 2i64, name: "Bob", dept_id: 20i64 }).unwrap();
//!
//! // 3. Define Virtual Relvar "DEPT_10_STAFF"
//! // This view filters employees to show only those in department 10.
//! let view_type = RelationType::new(
//!     TupleType::new()
//!         .with_attribute("id", ScalarType::Int)
//!         .with_attribute("name", ScalarType::String)
//!         .with_attribute("dept_id", ScalarType::Int)
//! );
//!
//! db.define_virtual_relvar(
//!     "DEPT_10_STAFF",
//!     view_type,
//!     |db: &dyn QueryExecutor| {
//!         // The closure receives a QueryExecutor (like the Database)
//!         // and returns a Relation.
//!         let employees = db.query("EMPLOYEES")?;
//!
//!         // Relational Algebra: Restrict to dept_id = 10
//!         Ok(employees.restrict(|t| {
//!             t.get_typed::<i64>("dept_id").unwrap_or(-1) == 10
//!         }))
//!     }
//! ).unwrap();
//!
//! // 4. Query the View
//! let result = db.query("DEPT_10_STAFF").unwrap();
//! assert_eq!(result.cardinality(), 1); // Only Alice
//!
//! // 5. Updates to base table reflect in view immediately
//! db.insert("EMPLOYEES", tuple! { id: 3i64, name: "Charlie", dept_id: 10i64 }).unwrap();
//!
//! let result_updated = db.query("DEPT_10_STAFF").unwrap();
//! assert_eq!(result_updated.cardinality(), 2); // Alice and Charlie
//! ```

use crate::error::DatabaseError;
use crate::traits::QueryExecutor;
use crate::types::RelationType;
use crate::values::Relation;

/// Definition of a virtual relvar (view).
///
/// Stores the metadata required to evaluate a virtual relvar on demand.
#[derive(Debug, Clone)]
pub struct VirtualRelvarDefinition {
    /// The unique name of the virtual relvar.
    pub name: String,

    /// The relation type (heading) of the view.
    ///
    /// This defines the schema of the result produced by the evaluator.
    /// The database uses this to validate queries against the view without
    /// needing to evaluate it first.
    pub relation_type: RelationType,

    /// The evaluation function (closure) that computes the view's contents.
    ///
    /// # Signature
    ///
    /// `fn(&dyn QueryExecutor) -> Result<Relation, DatabaseError>`
    ///
    /// - **Input**: A `&dyn QueryExecutor` trait object, which allows the view
    ///   to query other relvars (base or virtual) in the database.
    /// - **Output**: A `Result` containing the computed `Relation`.
    ///
    /// # Safety
    ///
    /// The evaluator is passed a read-only reference (`&dyn`), ensuring that
    /// viewing a relation cannot cause side effects (mutations) in the database.
    pub evaluator: fn(&dyn QueryExecutor) -> Result<Relation, DatabaseError>,
}
