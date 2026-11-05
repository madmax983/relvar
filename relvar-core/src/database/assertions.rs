//! Registering and removing database assertions (TTM RM Prescriptions 16
//! and 23).
//!
//! A database has two kinds of assertion:
//!
//! - **declarative** ([`DeclarativeAssertion`]): a relational boolean over
//!   query expressions. It is stored in the constraint catalog with the
//!   other constraints, so a reopened database enforces it without any
//!   re-registration.
//! - **closure** ([`DatabaseAssertion`]): an arbitrary Rust predicate. It
//!   cannot be serialized, so only its name is stored. A reopened database
//!   whose catalog names a closure assertion that has not been registered
//!   again refuses every change
//!   ([`DatabaseError::UnregisteredAssertion`]) until it is registered or
//!   explicitly forgotten: no assertion is ever silently lost.
//!
//! Both kinds share one name space and are enforced the same way: on
//! registration against the current state, and after every statement
//! against the candidate post-statement state (see the `statement` module).

use crate::constraints::assertion::{AssertionError, DatabaseAssertion, DeclarativeAssertion};
use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::StorageEngine;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

impl<E: StorageEngine> Database<E> {
    /// Registers a closure-backed database assertion.
    ///
    /// The assertion is validated against the current database state before
    /// it is accepted: if the state already violates the assertion, the
    /// assertion is rejected and the database is left unchanged. Once
    /// registered it is checked after every statement.
    ///
    /// TTM: RM Prescription 23 (the Golden Rule), for a constraint written
    /// in the host language.
    ///
    /// # Persistence
    ///
    /// An assertion is a Rust closure and cannot be stored in the catalog,
    /// unlike the declarative constraints, but its **name** is stored. A
    /// reopened persistent database whose catalog names a closure
    /// assertion that has not been registered again is *incomplete*: every
    /// change fails with [`DatabaseError::UnregisteredAssertion`] (queries
    /// keep working) until this method registers an assertion with that
    /// name — validated against the current data like any new one — or
    /// [`forget_assertion`](Self::forget_assertion) drops it. So an
    /// assertion is never silently lost. See
    /// [`unregistered_assertions`](Self::unregistered_assertions).
    ///
    /// Prefer [`add_declarative_assertion`](Self::add_declarative_assertion)
    /// whenever the rule can be written as a query condition: it is stored
    /// in full and needs no re-registration.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::{DatabaseAssertion, tuple};
    ///
    /// let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
    /// db.create_relvar("USERS", rel_type).unwrap();
    /// db.insert("USERS", tuple! { id: 1i64 }).unwrap();
    ///
    /// let assertion = DatabaseAssertion::new(
    ///     "users_non_empty",
    ///     "The USERS relvar must not be empty",
    ///     |db: &mut Database<InMemoryEngine>| db.query("USERS").unwrap().cardinality() > 0,
    /// );
    /// db.add_assertion(assertion).unwrap();
    /// assert_eq!(db.assertions().len(), 1);
    /// ```
    ///
    /// # Errors
    ///
    /// - [`DatabaseError::AssertionViolation`] if the assertion does not
    ///   hold against the current database state;
    /// - [`DatabaseError::Assertion`] wrapping
    ///   [`AssertionError::DuplicateName`] if an assertion of either kind
    ///   already has this name;
    /// - [`DatabaseError::TransactionError`] inside a transaction (a
    ///   rollback could restore a state the assertion was never checked
    ///   against);
    /// - a storage error if the engine cannot store the catalog (only for a
    ///   name the catalog does not record yet).
    pub fn add_assertion(&mut self, assertion: DatabaseAssertion<E>) -> Result<(), DatabaseError> {
        self.ensure_no_transaction("add_assertion")?;
        let name = assertion.name();
        let database = self.constraints.database_constraints();
        let declarative_clash = database.assertions.iter().any(|a| a.name() == name);
        let registered_clash = self.assertions.iter().any(|a| a.name() == name);
        if declarative_clash || registered_clash {
            return Err(AssertionError::DuplicateName(name.to_string()).into());
        }
        let recorded = database.closure_assertions.iter().any(|n| n == name);
        let was_evaluating = core::mem::replace(&mut self.evaluating_assertions, true);
        let satisfied = assertion.is_satisfied_by(self);
        self.evaluating_assertions = was_evaluating;
        if !satisfied {
            return Err(AssertionError::Violation {
                assertion_name: assertion.name().to_string(),
                description: assertion.description().to_string(),
            }
            .into());
        }
        if !recorded {
            // Record the name first, so a failed write leaves nothing
            // registered.
            let name = name.to_string();
            self.change_constraints(|manager, _| {
                manager
                    .database_constraints_mut()
                    .closure_assertions
                    .push(name);
                Ok(())
            })?;
        }
        self.assertions.push(assertion);
        Ok(())
    }

    /// Registers a declarative database assertion and stores it in the
    /// constraint catalog.
    ///
    /// TTM: RM Prescription 16 (a database contains its constraints) and
    /// RM Prescription 23 (the Golden Rule). The assertion is accepted only
    /// if every relvar it refers to is a base relvar, its condition is well
    /// typed, and the current state satisfies it. From then on it is
    /// enforced after every statement, like every other constraint, and
    /// checked by [`verify_integrity`](Self::verify_integrity). It is
    /// stored atomically with the other constraints
    /// ([`StorageEngine::store_constraints`]), so
    /// [`Database::new`](Self::new) restores it when a persistent database
    /// is reopened. A relvar it refers to cannot be dropped while it
    /// exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::constraints::{AssertionCondition, CmpOp, DeclarativeAssertion};
    /// use relvar_core::database::Database;
    /// use relvar_core::query::Query;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, ScalarType, TupleType};
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("id", ScalarType::Int);
    /// db.create_relvar("SEATS", RelationType::new(heading)).unwrap();
    ///
    /// db.add_declarative_assertion(DeclarativeAssertion::new(
    ///     "capacity",
    ///     "At most two seats",
    ///     AssertionCondition::Cardinality { query: Query::scan("SEATS"), comparison: CmpOp::Le, value: 2 },
    /// ))
    /// .unwrap();
    ///
    /// db.insert("SEATS", tuple! { id: 1i64 }).unwrap();
    /// db.insert("SEATS", tuple! { id: 2i64 }).unwrap();
    /// assert!(db.insert("SEATS", tuple! { id: 3i64 }).is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Nothing is changed when an error is returned.
    ///
    /// - [`DatabaseError::AssertionViolation`] if the current state violates
    ///   the assertion;
    /// - [`DatabaseError::Assertion`] wrapping
    ///   [`AssertionError::DuplicateName`] (the name is taken by an
    ///   assertion of either kind), [`AssertionError::UnknownRelvar`] (a
    ///   scanned name is not a base relvar) or
    ///   [`AssertionError::Evaluation`] (the condition is ill typed);
    /// - [`DatabaseError::TransactionError`] inside a transaction;
    /// - a storage error if the engine cannot store the catalog.
    pub fn add_declarative_assertion(
        &mut self,
        assertion: DeclarativeAssertion,
    ) -> Result<(), DatabaseError> {
        self.ensure_no_transaction("add_declarative_assertion")?;
        if self.assertion_name_in_use(assertion.name()) {
            return Err(AssertionError::DuplicateName(assertion.name().to_string()).into());
        }
        if let Some(relvar) = assertion
            .referenced_relvars()
            .into_iter()
            .find(|relvar| !self.engine.relation_exists(relvar))
        {
            return Err(AssertionError::UnknownRelvar {
                assertion_name: assertion.name().to_string(),
                relvar: relvar.to_string(),
            }
            .into());
        }
        assertion.check(self)?;
        self.change_constraints(|manager, _| {
            manager
                .database_constraints_mut()
                .assertions
                .push(assertion);
            Ok(())
        })
    }

    /// Removes the registered database assertion (of either kind) with the
    /// given name.
    ///
    /// Returns `Ok(true)` if an assertion with that name was registered and
    /// removed, `Ok(false)` otherwise (an unregistered closure assertion is
    /// dropped with [`forget_assertion`](Self::forget_assertion)). The
    /// reduced constraint catalog is stored: a removed declarative
    /// assertion, or a removed closure assertion's name, does not come back
    /// when the database is reopened.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::DatabaseAssertion;
    ///
    /// let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    /// db.add_assertion(DatabaseAssertion::new(
    ///     "trivially_true",
    ///     "Always holds",
    ///     |_: &mut Database<InMemoryEngine>| true,
    /// ))
    /// .unwrap();
    ///
    /// assert!(db.remove_assertion("trivially_true").unwrap());
    /// assert!(!db.remove_assertion("trivially_true").unwrap());
    /// assert_eq!(db.assertions().len(), 0);
    /// ```
    ///
    /// # Errors
    ///
    /// [`DatabaseError::TransactionError`] inside a transaction (constraint
    /// definitions are not transactional), or a storage error if the engine
    /// cannot store the catalog; the assertion is then still registered.
    pub fn remove_assertion(&mut self, name: &str) -> Result<bool, DatabaseError> {
        self.ensure_no_transaction("remove_assertion")?;
        if self
            .constraints
            .database_constraints()
            .assertions
            .iter()
            .any(|assertion| assertion.name() == name)
        {
            self.change_constraints(|manager, _| {
                manager
                    .database_constraints_mut()
                    .assertions
                    .retain(|assertion| assertion.name() != name);
                Ok(())
            })?;
            return Ok(true);
        }
        if !self
            .assertions
            .iter()
            .any(|assertion| assertion.name() == name)
        {
            return Ok(false);
        }
        self.change_constraints(|manager, _| {
            manager
                .database_constraints_mut()
                .closure_assertions
                .retain(|recorded| recorded != name);
            Ok(())
        })?;
        self.assertions.retain(|assertion| assertion.name() != name);
        Ok(true)
    }

    /// Drops a closure assertion that the catalog records but that has not
    /// been registered since the database was opened.
    ///
    /// TTM: RM Prescription 16. This is the explicit way to give up an
    /// assertion whose closure is no longer available: it removes the name
    /// from the stored catalog, so the database is no longer incomplete on
    /// its account (see [`add_assertion`](Self::add_assertion)). Returns
    /// `Ok(true)` if `name` was such an unregistered assertion, `Ok(false)`
    /// otherwise; a registered assertion is removed with
    /// [`remove_assertion`](Self::remove_assertion) instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// assert!(!db.forget_assertion("never_recorded").unwrap());
    /// ```
    ///
    /// # Errors
    ///
    /// [`DatabaseError::TransactionError`] inside a transaction, or a
    /// storage error if the engine cannot store the catalog; the name is
    /// then still recorded.
    pub fn forget_assertion(&mut self, name: &str) -> Result<bool, DatabaseError> {
        self.ensure_no_transaction("forget_assertion")?;
        if !self.unregistered_assertion_names().any(|n| n == name) {
            return Ok(false);
        }
        self.change_constraints(|manager, _| {
            manager
                .database_constraints_mut()
                .closure_assertions
                .retain(|recorded| recorded != name);
            Ok(())
        })?;
        Ok(true)
    }

    /// The names of the closure assertions the catalog records but that
    /// have not been registered since the database was opened, in
    /// registration order.
    ///
    /// While this is not empty the database refuses every change with
    /// [`DatabaseError::UnregisteredAssertion`]; register each with
    /// [`add_assertion`](Self::add_assertion) or drop it with
    /// [`forget_assertion`](Self::forget_assertion).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// let db = Database::new(InMemoryEngine::new());
    /// assert!(db.unregistered_assertions().is_empty());
    /// ```
    pub fn unregistered_assertions(&self) -> Vec<String> {
        self.unregistered_assertion_names()
            .map(ToString::to_string)
            .collect()
    }

    /// See [`unregistered_assertions`](Self::unregistered_assertions).
    ///
    /// While assertion predicates run, the registered closures are moved
    /// out of `self` (see `evaluate_assertions`), so registration cannot be
    /// told from the list: nothing is reported then. Every statement
    /// checked completeness before it got that far, and DML from inside a
    /// predicate is refused by the statement guard anyway.
    fn unregistered_assertion_names(&self) -> impl Iterator<Item = &str> {
        let recorded: &[String] = if self.evaluating_assertions {
            &[]
        } else {
            &self.constraints.database_constraints().closure_assertions
        };
        recorded
            .iter()
            .map(String::as_str)
            .filter(|name| !self.assertions.iter().any(|a| a.name() == *name))
    }

    /// Fails with [`DatabaseError::UnregisteredAssertion`] (naming the
    /// first one) while a recorded closure assertion is unregistered.
    ///
    /// Every statement that changes the database's data, relvars or
    /// relvar constraints calls this first: insert, bulk insert, delete,
    /// update, multiple assignment, create/drop relvar and the
    /// `set_*_constraints` methods. Queries, transaction control, and
    /// registering, removing or forgetting assertions do not, so the
    /// database can be brought back to a complete state.
    pub(crate) fn ensure_complete(&self) -> Result<(), DatabaseError> {
        match self.unregistered_assertion_names().next() {
            Some(name) => Err(DatabaseError::UnregisteredAssertion(name.to_string())),
            None => Ok(()),
        }
    }

    /// Returns the closure-backed database assertions registered on this
    /// database.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::DatabaseAssertion;
    ///
    /// let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());
    /// assert_eq!(db.assertions().len(), 0);
    ///
    /// db.add_assertion(DatabaseAssertion::new(
    ///     "trivially_true",
    ///     "Always holds",
    ///     |_: &mut Database<InMemoryEngine>| true,
    /// ))
    /// .unwrap();
    /// assert_eq!(db.assertions()[0].name(), "trivially_true");
    /// ```
    pub fn assertions(&self) -> &[DatabaseAssertion<E>] {
        &self.assertions
    }

    /// Returns the declarative database assertions of this database, in
    /// registration order (including those restored from the catalog).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    ///
    /// let db = Database::new(InMemoryEngine::new());
    /// assert!(db.declarative_assertions().is_empty());
    /// ```
    pub fn declarative_assertions(&self) -> &[DeclarativeAssertion] {
        &self.constraints.database_constraints().assertions
    }

    /// Whether any assertion of either kind is registered, i.e. whether a
    /// statement must be guarded so it can be undone after its writes.
    pub(crate) fn has_assertions(&self) -> bool {
        !self.assertions.is_empty() || !self.declarative_assertions().is_empty()
    }

    /// Whether `name` is taken by an assertion of either kind.
    fn assertion_name_in_use(&self, name: &str) -> bool {
        self.constraints.database_constraints().has_name(name)
            || self
                .assertions
                .iter()
                .any(|assertion| assertion.name() == name)
    }

    /// Fails if a declarative assertion refers to `relvar`, which therefore
    /// cannot be dropped.
    pub(crate) fn ensure_not_referenced_by_assertion(
        &self,
        relvar: &str,
    ) -> Result<(), AssertionError> {
        match self
            .constraints
            .database_constraints()
            .referenced_relvars()
            .find(|(referenced, _)| *referenced == relvar)
        {
            Some((_, assertion_name)) => Err(AssertionError::RelvarReferenced {
                relvar: relvar.to_string(),
                assertion_name: assertion_name.to_string(),
            }),
            None => Ok(()),
        }
    }
}
