//! Database assertions (cross-relvar integrity constraints).
//!
//! This module implements database assertions per TTM RM Prescription 9.
//! Unlike CHECK constraints (which validate individual tuples), database
//! assertions are predicates that must hold across the *entire database
//! state*, potentially spanning multiple relvars.
//!
//! There are two kinds:
//!
//! - [`DeclarativeAssertion`]: a named [`AssertionCondition`] over
//!   [`Query`] expressions (`IS_EMPTY`, subset, equality, cardinality).
//!   It is plain data, so it is stored in the constraint catalog and a
//!   reopened database enforces it with nothing to re-register.
//! - [`DatabaseAssertion`]: a named Rust closure, for rules the query
//!   language cannot express. It cannot be stored.
//!
//! # TTM Compliance
//!
//! - TTM RM Prescription 9: General integrity constraints (database assertions)
//! - TTM RM Prescription 16: a database contains its constraints
//!   (declarative assertions are persisted)
//! - TTM RM Prescription 23: the Golden Rule (both kinds are checked at
//!   every statement boundary)
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

use crate::constraints::CmpOp;
use crate::database::Database;
use crate::error::DatabaseError;
use crate::query::{Query, QueryError};
use crate::storage_engine::StorageEngine;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur when a database assertion is registered,
/// evaluated or removed.
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

    /// A [`DeclarativeAssertion`]'s condition could not be evaluated: one
    /// of its queries is ill typed (for example a `UNION` of different
    /// headings, or a predicate comparing values of different types).
    ///
    /// The statement (or registration) that needed the evaluation is
    /// rejected: a constraint that cannot be evaluated cannot be shown to
    /// hold.
    #[error("Database assertion '{assertion_name}' could not be evaluated: {reason}")]
    Evaluation {
        /// Name of the assertion.
        assertion_name: String,
        /// Why evaluation failed.
        reason: String,
    },

    /// An assertion with this name is already registered (or, for a
    /// closure assertion, recorded in the catalog). Assertion names are
    /// unique across both kinds of assertion.
    #[error("A database assertion named '{0}' already exists")]
    DuplicateName(String),

    /// A [`DeclarativeAssertion`] refers to a name that is not a base
    /// relvar. Stored assertions may refer only to base relvars: virtual
    /// relvars are not part of the stored catalog.
    #[error(
        "Database assertion '{assertion_name}' refers to '{relvar}', which is not a base relvar"
    )]
    UnknownRelvar {
        /// Name of the assertion.
        assertion_name: String,
        /// The name that is not a base relvar.
        relvar: String,
    },

    /// The relvar cannot be dropped: a [`DeclarativeAssertion`] refers to
    /// it. Remove the assertion first.
    #[error("Relvar '{relvar}' is referenced by database assertion '{assertion_name}'")]
    RelvarReferenced {
        /// The relvar being dropped.
        relvar: String,
        /// The assertion that refers to it.
        assertion_name: String,
    },
}

impl From<AssertionError> for DatabaseError {
    /// A violation becomes [`DatabaseError::AssertionViolation`] (as it
    /// always has); every other assertion error becomes
    /// [`DatabaseError::Assertion`].
    fn from(error: AssertionError) -> Self {
        match error {
            AssertionError::Violation { .. } => {
                DatabaseError::AssertionViolation(error.to_string())
            }
            other => DatabaseError::Assertion(other),
        }
    }
}

/// A relational boolean over [`Query`] expressions: the condition of a
/// [`DeclarativeAssertion`].
///
/// TTM: RM Prescription 23. Any database constraint can be written as
/// `IS_EMPTY(expression)` — "no tuple breaks the rule" — and the other forms
/// are conveniences with the obvious meaning (`Subset(a, b)` is
/// `IS_EMPTY(a MINUS b)`). Because a condition is plain data, it is stored in
/// the catalog with the database (RM Prescription 16), unlike a closure.
///
/// # Type rules
///
/// Every query must be well typed, and the operands of
/// [`Subset`](Self::Subset) and [`Equal`](Self::Equal) must have the same
/// heading. A violation of either is an evaluation error, never a silent
/// `false`.
///
/// # Examples
///
/// ```
/// use relvar_core::constraints::AssertionCondition;
/// use relvar_core::query::Query;
///
/// // Every shipment refers to an existing supplier.
/// let condition =
///     AssertionCondition::IsEmpty(Query::scan("SP").not_matching(Query::scan("S")));
/// assert_eq!(condition.referenced_relvars(), vec!["S", "SP"]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssertionCondition {
    /// The query's result has no tuples (`IS_EMPTY`).
    IsEmpty(Query),
    /// The query's result has at least one tuple (`NOT IS_EMPTY`).
    NotEmpty(Query),
    /// Every tuple of the first query's result is a tuple of the second's
    /// (`a ⊆ b`). The headings must be the same.
    Subset(Query, Query),
    /// The two queries' results are equal (`a = b`). The headings must be
    /// the same.
    Equal(Query, Query),
    /// The cardinality of the query's result compares with `value` as
    /// `comparison` says, e.g. `COUNT(P) <= 10`.
    Cardinality {
        /// The query whose result is counted.
        query: Query,
        /// How the count compares with `value` (`count <op> value`).
        comparison: CmpOp,
        /// The number the count is compared with.
        value: u64,
    },
}

impl AssertionCondition {
    /// Evaluates the condition against the database's current state.
    ///
    /// # Errors
    ///
    /// Returns the [`QueryError`] of an ill-typed query, including
    /// [`QueryError::Difference`] when the operands of `Subset` or `Equal`
    /// have different headings.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::constraints::AssertionCondition;
    /// use relvar_core::database::Database;
    /// use relvar_core::query::Query;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, ScalarType, TupleType};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("USERS", RelationType::new(heading)).unwrap();
    ///
    /// let condition = AssertionCondition::IsEmpty(Query::scan("USERS"));
    /// assert!(condition.is_satisfied_by(&db).unwrap());
    /// ```
    pub fn is_satisfied_by<E: StorageEngine>(&self, db: &Database<E>) -> Result<bool, QueryError> {
        match self {
            AssertionCondition::IsEmpty(query) => Ok(query.execute(db)?.is_empty()),
            AssertionCondition::NotEmpty(query) => Ok(!query.execute(db)?.is_empty()),
            AssertionCondition::Subset(subset, superset) => {
                let subset = subset.execute(db)?;
                Ok(subset.minus_into(&superset.execute(db)?)?.is_empty())
            }
            AssertionCondition::Equal(left, right) => {
                let left = left.execute(db)?;
                let right = right.execute(db)?;
                // `left MINUS right` checks the headings; once `left` is a
                // subset of `right`, equal cardinalities make them equal.
                let cardinality = left.cardinality();
                Ok(cardinality == right.cardinality() && left.minus_into(&right)?.is_empty())
            }
            AssertionCondition::Cardinality {
                query,
                comparison,
                value,
            } => {
                let count = u64::try_from(query.execute(db)?.cardinality()).unwrap_or(u64::MAX);
                Ok(match comparison {
                    CmpOp::Eq => count == *value,
                    CmpOp::Ne => count != *value,
                    CmpOp::Lt => count < *value,
                    CmpOp::Le => count <= *value,
                    CmpOp::Gt => count > *value,
                    CmpOp::Ge => count >= *value,
                })
            }
        }
    }

    /// The relvar names the condition's queries scan, sorted and without
    /// repetitions.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::constraints::AssertionCondition;
    /// use relvar_core::query::Query;
    ///
    /// let condition = AssertionCondition::Subset(Query::scan("B"), Query::scan("A").union(Query::scan("B")));
    /// assert_eq!(condition.referenced_relvars(), vec!["A", "B"]);
    /// ```
    pub fn referenced_relvars(&self) -> Vec<&str> {
        let mut names = match self {
            AssertionCondition::IsEmpty(query)
            | AssertionCondition::NotEmpty(query)
            | AssertionCondition::Cardinality { query, .. } => query.referenced_relvars(),
            AssertionCondition::Subset(left, right) | AssertionCondition::Equal(left, right) => {
                let mut names = left.referenced_relvars();
                names.extend(right.referenced_relvars());
                names
            }
        };
        names.sort_unstable();
        names.dedup();
        names
    }
}

/// A database assertion whose condition is a relational expression, so it
/// can be stored with the database.
///
/// TTM: RM Prescription 16 (a database contains its constraints) and RM
/// Prescription 23 (the Golden Rule). A declarative assertion is a named
/// [`AssertionCondition`] over the database's base relvars. Once registered
/// with [`Database::add_declarative_assertion`] it is
///
/// - validated against the current state (and rejected if violated);
/// - enforced at every statement boundary — insert, bulk insert, delete,
///   update and multiple assignment, inside and outside transactions —
///   exactly like a closure [`DatabaseAssertion`];
/// - checked by [`Database::verify_integrity`];
/// - stored in the constraint catalog, so a reopened persistent database
///   enforces it with no re-registration.
///
/// A relvar an assertion refers to cannot be dropped while the assertion
/// exists.
///
/// # Examples
///
/// ```
/// use relvar_core::constraints::{AssertionCondition, DeclarativeAssertion};
/// use relvar_core::database::Database;
/// use relvar_core::error::DatabaseError;
/// use relvar_core::query::Query;
/// use relvar_core::storage_engine::InMemoryEngine;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::tuple;
///
/// let mut db = Database::new(InMemoryEngine::new());
/// db.create_relvar("S", RelationType::new(TupleType::new().with_attribute("sno", ScalarType::Int))).unwrap();
/// db.create_relvar(
///     "SP",
///     RelationType::new(
///         TupleType::new()
///             .with_attribute("sno", ScalarType::Int)
///             .with_attribute("pno", ScalarType::Int),
///     ),
/// )
/// .unwrap();
///
/// // IS_EMPTY(SP NOT MATCHING S): every shipment has a supplier.
/// db.add_declarative_assertion(DeclarativeAssertion::new(
///     "shipment_has_supplier",
///     "Every shipment refers to an existing supplier",
///     AssertionCondition::IsEmpty(Query::scan("SP").not_matching(Query::scan("S"))),
/// ))
/// .unwrap();
///
/// assert!(matches!(
///     db.insert("SP", tuple! { sno: 1i64, pno: 7i64 }),
///     Err(DatabaseError::AssertionViolation(_))
/// ));
/// db.insert("S", tuple! { sno: 1i64 }).unwrap();
/// db.insert("SP", tuple! { sno: 1i64, pno: 7i64 }).unwrap();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclarativeAssertion {
    name: String,
    description: String,
    condition: AssertionCondition,
}

impl DeclarativeAssertion {
    /// Creates a declarative assertion.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::constraints::{AssertionCondition, DeclarativeAssertion};
    /// use relvar_core::query::Query;
    ///
    /// let assertion = DeclarativeAssertion::new(
    ///     "has_admin",
    ///     "There is at least one administrator",
    ///     AssertionCondition::NotEmpty(Query::scan("ADMINS")),
    /// );
    /// assert_eq!(assertion.name(), "has_admin");
    /// ```
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        condition: AssertionCondition,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            condition,
        }
    }

    /// The assertion's name, unique among the database's assertions.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The human-readable description of the rule.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The assertion's condition.
    pub fn condition(&self) -> &AssertionCondition {
        &self.condition
    }

    /// The relvars the condition refers to, sorted and without repetitions.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::constraints::{AssertionCondition, DeclarativeAssertion};
    /// use relvar_core::query::Query;
    ///
    /// let assertion = DeclarativeAssertion::new(
    ///     "disjoint",
    ///     "No id is both active and archived",
    ///     AssertionCondition::IsEmpty(Query::scan("ACTIVE").intersect(Query::scan("ARCHIVED"))),
    /// );
    /// assert_eq!(assertion.referenced_relvars(), vec!["ACTIVE", "ARCHIVED"]);
    /// ```
    pub fn referenced_relvars(&self) -> Vec<&str> {
        self.condition.referenced_relvars()
    }

    /// Checks the assertion against the database's current state.
    ///
    /// # Errors
    ///
    /// [`AssertionError::Violation`] if the condition does not hold, or
    /// [`AssertionError::Evaluation`] if it cannot be evaluated.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::constraints::{AssertionCondition, DeclarativeAssertion};
    /// use relvar_core::database::Database;
    /// use relvar_core::query::Query;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, ScalarType, TupleType};
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// db.create_relvar("T", RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int))).unwrap();
    /// let not_empty = DeclarativeAssertion::new("t_not_empty", "T has a tuple", AssertionCondition::NotEmpty(Query::scan("T")));
    /// assert!(not_empty.check(&db).is_err());
    /// ```
    pub fn check<E: StorageEngine>(&self, db: &Database<E>) -> Result<(), AssertionError> {
        match self.condition.is_satisfied_by(db) {
            Ok(true) => Ok(()),
            Ok(false) => Err(AssertionError::Violation {
                assertion_name: self.name.clone(),
                description: self.description.clone(),
            }),
            Err(error) => Err(AssertionError::Evaluation {
                assertion_name: self.name.clone(),
                reason: error.to_string(),
            }),
        }
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
/// The predicate is an arbitrary Rust closure, so it cannot be serialized
/// into the catalog. Unlike keys, foreign keys, CHECK and type constraints
/// and [`DeclarativeAssertion`]s — which a persistent database stores and
/// [`Database::new`] restores — a closure assertion is **not persisted**:
/// it must be re-registered with [`Database::add_assertion`] every time a
/// database is opened, before it is modified. Prefer a
/// [`DeclarativeAssertion`] whenever the rule can be written as a query
/// condition.
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

        assert!(db.remove_assertion("total_non_negative").unwrap());
        assert_eq!(db.assertions().len(), 0);
    }

    #[test]
    fn test_remove_assertion_missing_returns_false() {
        let mut db = accounts_db();
        assert!(!db.remove_assertion("no_such_assertion").unwrap());
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
