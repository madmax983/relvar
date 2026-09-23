//! Database assertions (cross-relvar integrity constraints).
//!
//! This module implements database assertions per TTM RM Prescription 9.
//! Unlike CHECK constraints (which validate individual tuples), database
//! assertions are predicates that must hold across the *entire database
//! state*, potentially spanning multiple relvars.
//!
//! # TTM Compliance
//!
//! - TTM RM Prescription 9: General integrity constraints (database assertions)
//! - Predicates are pure queries over relvars (no NULL involvement)
//!
//! # Example
//!
//! ```
//! use relvar_core::database::Database;
//! use relvar_core::storage_engine::InMemoryEngine;
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use relvar_core::{DatabaseAssertion, tuple};
//! use relvar_core::values::ScalarValue;
//!
//! let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
//! let rel_type = RelationType::new(
//!     TupleType::new()
//!         .with_attribute("id", ScalarType::Int)
//!         .with_attribute("balance", ScalarType::Int),
//! );
//! db.create_relvar("ACCOUNTS", rel_type).unwrap();
//! db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 100i64 }).unwrap();
//!
//! // The sum of all balances must stay non-negative.
//! let assertion = DatabaseAssertion::new(
//!     "total_non_negative",
//!     "Sum of all account balances must be non-negative",
//!     |db: &mut Database<InMemoryEngine>| {
//!         let accounts = db.query("ACCOUNTS").unwrap();
//!         let total: i64 = accounts
//!             .tuples()
//!             .filter_map(|t| t.get("balance"))
//!             .filter_map(|v| {
//!                 if let ScalarValue::Int(n) = v {
//!                     Some(*n)
//!                 } else {
//!                     None
//!                 }
//!             })
//!             .sum();
//!         total >= 0
//!     },
//! );
//! assert!(assertion.is_satisfied_by(&mut db));
//!
//! // Registering validates against the current state and enforces the
//! // assertion on every later INSERT, UPDATE, and DELETE.
//! db.add_assertion(assertion).unwrap();
//! assert_eq!(db.assertions().len(), 1);
//! ```

use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use thiserror::Error;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::string::ToString;

/// Errors that can occur when a database assertion is evaluated.
#[derive(Debug, Error)]
pub enum AssertionError {
    /// A database assertion was violated.
    #[error("Database assertion '{assertion_name}' violated: {description}")]
    Violation {
        /// Name of the violated assertion.
        assertion_name: String,
        /// Human-readable description of the business rule that was broken.
        description: String,
    },
}

impl From<AssertionError> for DatabaseError {
    fn from(error: AssertionError) -> Self {
        DatabaseError::AssertionViolation(error.to_string())
    }
}

/// The predicate backing a [`DatabaseAssertion`]: a boxed closure evaluated
/// against the whole database state.
///
/// (A named alias keeps the struct field readable and satisfies
/// `clippy::type_complexity`.)
type AssertionPredicate<E> = Box<dyn Fn(&mut Database<E>) -> bool + Send + Sync>;

/// A database assertion that must hold across the entire database state.
///
/// TTM: RM Prescription 9 - Database-level integrity constraints.
///
/// A database assertion is a named predicate over the whole database. Unlike
/// [`CheckConstraint`](crate::constraints::CheckConstraint)s, which validate
/// single tuples, an assertion may reference multiple relvars and use
/// arbitrary Rust logic (aggregations, joins, cardinality checks), making it
/// the right tool for cross-relvar business rules such as "total debits equal
/// total credits" or "every employee references an existing department".
///
/// Assertions are enforced at two points:
/// - [`Database::add_assertion`] validates the assertion against the current
///   state and rejects it if it is already violated.
/// - [`Database::insert`], [`Database::update`], and [`Database::delete`]
///   re-check every assertion after the operation; the first violation rolls
///   the operation back and reports [`DatabaseError::AssertionViolation`].
///
/// # Non-serializable predicates
///
/// The predicate is an arbitrary Rust closure, so assertions cannot be
/// serialized into the catalog. They must be re-registered after every
/// database restart.
///
/// # Predicate discipline
///
/// Predicates receive `&mut Database<E>` so they can query any relvar, but
/// they must be read-only: performing DML (insert/update/delete) inside a
/// predicate is rejected with [`DatabaseError::TransactionError`].
///
/// # Examples
///
/// ```
/// use relvar_core::database::Database;
/// use relvar_core::storage_engine::InMemoryEngine;
/// use relvar_core::{DatabaseAssertion, tuple};
/// use relvar_core::types::{RelationType, TupleType, ScalarType};
///
/// let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
/// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
/// db.create_relvar("USERS", rel_type).unwrap();
///
/// let assertion = DatabaseAssertion::new(
///     "users_non_empty",
///     "The USERS relvar must not be empty",
///     |db: &mut Database<InMemoryEngine>| db.query("USERS").unwrap().cardinality() > 0,
/// );
/// assert_eq!(assertion.name(), "users_non_empty");
/// assert_eq!(assertion.description(), "The USERS relvar must not be empty");
/// ```
pub struct DatabaseAssertion<E: StorageEngine> {
    name: String,
    description: String,
    predicate: AssertionPredicate<E>,
}

impl<E: StorageEngine> DatabaseAssertion<E> {
    /// Creates a new database assertion.
    ///
    /// The predicate is evaluated against the database state whenever the
    /// assertion is registered and after every DML operation. It must return
    /// `true` when the business rule holds and `false` when it is violated.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::DatabaseAssertion;
    ///
    /// let assertion: DatabaseAssertion<InMemoryEngine> = DatabaseAssertion::new(
    ///     "total_balance_positive",
    ///     "Total of all account balances must be non-negative",
    ///     |_: &mut Database<InMemoryEngine>| true,
    /// );
    /// assert_eq!(assertion.name(), "total_balance_positive");
    /// ```
    pub fn new<F>(name: impl Into<String>, description: impl Into<String>, predicate: F) -> Self
    where
        F: Fn(&mut Database<E>) -> bool + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            predicate: Box::new(predicate),
        }
    }

    /// Returns the assertion's unique name.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the human-readable description of the business rule.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Evaluates the assertion predicate against the given database state.
    ///
    /// Returns `true` when the assertion holds, `false` when it is violated.
    ///
    /// # Examples
    ///
    /// ```text
    /// // Example
    /// ```
    pub fn is_satisfied_by(&self, db: &mut Database<E>) -> bool {
        (self.predicate)(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::error::DatabaseError;
    use crate::storage_engine::InMemoryEngine;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::ScalarValue;

    fn accounts_db() -> Database<InMemoryEngine> {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        let rel_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("balance", ScalarType::Int),
        );
        db.create_relvar("ACCOUNTS", rel_type).unwrap();
        db
    }

    fn total_balance_assertion() -> DatabaseAssertion<InMemoryEngine> {
        DatabaseAssertion::new(
            "total_non_negative",
            "Sum of all account balances must be non-negative",
            |db: &mut Database<InMemoryEngine>| {
                db.query("ACCOUNTS")
                    .map(|accounts| {
                        accounts
                            .tuples()
                            .filter_map(|t| t.get("balance"))
                            .filter_map(|v| {
                                if let ScalarValue::Int(n) = v {
                                    Some(*n)
                                } else {
                                    None
                                }
                            })
                            .sum::<i64>()
                            >= 0
                    })
                    .unwrap_or(false)
            },
        )
    }

    #[test]
    fn test_database_assertion_creation() {
        let assertion = total_balance_assertion();
        assert_eq!(assertion.name(), "total_non_negative");
        assert_eq!(
            assertion.description(),
            "Sum of all account balances must be non-negative"
        );
    }

    #[test]
    fn test_database_assertion_satisfied() {
        let mut db = accounts_db();
        db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 100i64 })
            .unwrap();
        db.insert("ACCOUNTS", tuple! { id: 2i64, balance: 50i64 })
            .unwrap();

        let assertion = total_balance_assertion();
        assert!(assertion.is_satisfied_by(&mut db));
    }

    #[test]
    fn test_database_assertion_violated() {
        let mut db = accounts_db();
        db.insert("ACCOUNTS", tuple! { id: 1i64, balance: -100i64 })
            .unwrap();

        let assertion = total_balance_assertion();
        assert!(!assertion.is_satisfied_by(&mut db));
    }

    #[test]
    fn test_add_assertion_accepts_satisfied_state() {
        let mut db = accounts_db();
        db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 100i64 })
            .unwrap();

        let result = db.add_assertion(total_balance_assertion());
        assert!(result.is_ok());
        assert_eq!(db.assertions().len(), 1);
        assert_eq!(db.assertions()[0].name(), "total_non_negative");
    }

    #[test]
    fn test_add_assertion_validates_existing_state() {
        let mut db = accounts_db();
        db.insert("ACCOUNTS", tuple! { id: 1i64, balance: -100i64 })
            .unwrap();

        let result = db.add_assertion(total_balance_assertion());
        assert!(result.is_err());
        assert!(matches!(result, Err(DatabaseError::AssertionViolation(_))));
        // Rejected assertion must not be registered.
        assert_eq!(db.assertions().len(), 0);
    }

    #[test]
    fn test_add_assertion_violation_message_includes_name_and_description() {
        let mut db = accounts_db();
        db.insert("ACCOUNTS", tuple! { id: 1i64, balance: -100i64 })
            .unwrap();

        let result = db.add_assertion(total_balance_assertion());
        let message = match result {
            Err(DatabaseError::AssertionViolation(message)) => message,
            _ => panic!("Expected AssertionViolation"),
        };
        assert!(
            message.contains("total_non_negative"),
            "message names the assertion: {message}"
        );
        assert!(
            message.contains("Sum of all account balances must be non-negative"),
            "message describes the rule: {message}"
        );
    }

    #[test]
    fn test_cross_relvar_assertion() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        let dept_type = RelationType::new(
            TupleType::new()
                .with_attribute("dept_id", ScalarType::Int)
                .with_attribute("name", ScalarType::String),
        );
        let emp_type = RelationType::new(
            TupleType::new()
                .with_attribute("emp_id", ScalarType::Int)
                .with_attribute("dept_id", ScalarType::Int),
        );
        db.create_relvar("DEPARTMENTS", dept_type).unwrap();
        db.create_relvar("EMPLOYEES", emp_type).unwrap();
        db.insert("DEPARTMENTS", tuple! { dept_id: 10i64, name: "Eng" })
            .unwrap();
        db.insert("EMPLOYEES", tuple! { emp_id: 1i64, dept_id: 10i64 })
            .unwrap();

        let assertion = DatabaseAssertion::new(
            "employees_have_valid_departments",
            "Every employee must reference an existing department",
            |db: &mut Database<InMemoryEngine>| {
                let employees = db.query("EMPLOYEES").unwrap();
                let departments = db.query("DEPARTMENTS").unwrap();
                employees.tuples().all(|emp| {
                    let dept_id = emp.get("dept_id");
                    departments
                        .tuples()
                        .any(|dept| dept.get("dept_id") == dept_id)
                })
            },
        );

        assert!(assertion.is_satisfied_by(&mut db));

        // An employee referencing a missing department violates the assertion.
        db.insert("EMPLOYEES", tuple! { emp_id: 2i64, dept_id: 99i64 })
            .unwrap();
        assert!(!assertion.is_satisfied_by(&mut db));
    }

    #[test]
    fn test_aggregate_assertion() {
        let mut db = accounts_db();
        db.insert("ACCOUNTS", tuple! { id: 1i64, balance: 100i64 })
            .unwrap();
        db.insert("ACCOUNTS", tuple! { id: 2i64, balance: 200i64 })
            .unwrap();

        let assertion = total_balance_assertion();
        assert!(assertion.is_satisfied_by(&mut db));

        // The aggregate holds on an empty relvar too (sum of nothing is 0).
        let mut empty_db = accounts_db();
        assert!(assertion.is_satisfied_by(&mut empty_db));
    }

    #[test]
    fn test_cardinality_assertion() {
        let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
        let user_type = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("role", ScalarType::String),
        );
        db.create_relvar("USERS", user_type).unwrap();
        db.insert("USERS", tuple! { id: 1i64, role: "admin" })
            .unwrap();
        db.insert("USERS", tuple! { id: 2i64, role: "viewer" })
            .unwrap();

        let assertion = DatabaseAssertion::new(
            "at_least_one_admin",
            "Database must have at least one admin user",
            |db: &mut Database<InMemoryEngine>| {
                db.query("USERS").unwrap().tuples().any(|user| {
                    matches!(
                        user.get("role"),
                        Some(ScalarValue::String(role)) if role == "admin"
                    )
                })
            },
        );

        assert!(assertion.is_satisfied_by(&mut db));
        assert!(db.add_assertion(assertion).is_ok());
        assert_eq!(db.assertions().len(), 1);
    }

    #[test]
    fn test_remove_assertion() {
        let mut db = accounts_db();
        db.add_assertion(total_balance_assertion()).unwrap();
        assert_eq!(db.assertions().len(), 1);

        assert!(db.remove_assertion("total_non_negative"));
        assert_eq!(db.assertions().len(), 0);
    }

    #[test]
    fn test_remove_assertion_missing_returns_false() {
        let mut db = accounts_db();
        assert!(!db.remove_assertion("no_such_assertion"));
    }

    #[test]
    fn test_assertions_list_returns_all_registered() {
        let mut db = accounts_db();
        db.add_assertion(total_balance_assertion()).unwrap();
        db.add_assertion(DatabaseAssertion::new(
            "always_true",
            "Trivially satisfied assertion",
            |_: &mut Database<InMemoryEngine>| true,
        ))
        .unwrap();

        let names: Vec<&str> = db.assertions().iter().map(|a| a.name()).collect();
        assert_eq!(names, vec!["total_non_negative", "always_true"]);
    }
}
