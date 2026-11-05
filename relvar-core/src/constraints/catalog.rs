//! A serializable snapshot of a database's constraints.
//!
//! TTM: RM Prescription 16 — a database contains its relvars *and* their
//! constraints. A [`ConstraintCatalog`] is the form in which a
//! [`ConstraintManager`](crate::constraints::ConstraintManager) hands the
//! database's constraints to a
//! [`StorageEngine`](crate::storage_engine::StorageEngine) for persistence,
//! and in which the engine hands them back when the database is reopened.
//!
//! It has two sections:
//!
//! - **per relvar** ([`RelvarConstraints`]): keys, foreign keys, CHECK and
//!   type constraints;
//! - **database-wide** ([`DatabaseConstraints`]): declarative assertions
//!   ([`DeclarativeAssertion`]), stored in full.
//!
//! Closure-backed assertions
//! ([`DatabaseAssertion`](crate::constraints::DatabaseAssertion)) cannot be
//! serialized.

use crate::constraints::{
    AttributeConstraints, CheckConstraints, DeclarativeAssertion, ForeignKeyConstraints,
    KeyConstraints,
};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// The declarative constraints of one relvar.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelvarConstraints {
    /// Primary and candidate keys, if declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<KeyConstraints>,
    /// Foreign keys, if declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreign_keys: Option<ForeignKeyConstraints>,
    /// Type constraints, one entry per constrained attribute, ordered by
    /// attribute name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<AttributeConstraints>,
    /// CHECK (tuple-level) constraints, if declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<CheckConstraints>,
}

impl RelvarConstraints {
    /// Returns `true` if no constraint of any kind is recorded.
    pub fn is_empty(&self) -> bool {
        self.keys.is_none()
            && self.foreign_keys.is_none()
            && self.attributes.is_empty()
            && self.checks.is_none()
    }
}

/// The database-wide constraints: those not declared on a single relvar.
///
/// TTM: RM Prescriptions 16 and 23. Every field defaults to empty when it
/// is missing, so catalogs written before database-wide constraints were
/// stored still load.
///
/// # Examples
///
/// ```
/// use relvar_core::constraints::{AssertionCondition, DatabaseConstraints, DeclarativeAssertion};
/// use relvar_core::query::Query;
///
/// let mut database = DatabaseConstraints::default();
/// assert!(database.is_empty());
/// database.assertions.push(DeclarativeAssertion::new(
///     "has_admin",
///     "There is an administrator",
///     AssertionCondition::NotEmpty(Query::scan("ADMINS")),
/// ));
/// assert!(!database.is_empty());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatabaseConstraints {
    /// Declarative assertions, in registration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assertions: Vec<DeclarativeAssertion>,
    /// The names of the closure-backed assertions
    /// ([`DatabaseAssertion`](crate::constraints::DatabaseAssertion)), in
    /// registration order. The closures themselves cannot be stored; the
    /// names let a reopened database know which ones must be registered
    /// again before it may be changed, so none is silently lost.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closure_assertions: Vec<String>,
}

impl DatabaseConstraints {
    /// Returns `true` if no database-wide constraint is recorded.
    pub fn is_empty(&self) -> bool {
        self.assertions.is_empty() && self.closure_assertions.is_empty()
    }

    /// Whether any database-wide constraint (declarative assertion or
    /// recorded closure assertion) is named `name`.
    pub fn has_name(&self, name: &str) -> bool {
        self.assertions.iter().any(|a| a.name() == name)
            || self
                .closure_assertions
                .iter()
                .any(|recorded| recorded == name)
    }

    /// Every relvar a database-wide constraint refers to, with the name of
    /// the constraint, in registration order. Names may repeat.
    pub fn referenced_relvars(&self) -> impl Iterator<Item = (&str, &str)> {
        self.assertions.iter().flat_map(|assertion| {
            assertion
                .referenced_relvars()
                .into_iter()
                .map(move |relvar| (relvar, assertion.name()))
        })
    }
}

/// The constraints of a whole database: per-relvar constraints keyed by
/// relvar name, and the database-wide constraints.
///
/// Produced by
/// [`ConstraintManager::to_catalog`](crate::constraints::ConstraintManager::to_catalog)
/// and consumed by
/// [`ConstraintManager::from_catalog`](crate::constraints::ConstraintManager::from_catalog).
/// Relvars are kept in name order, so the serialized form is deterministic.
///
/// # Examples
///
/// ```
/// use relvar_core::constraints::{ConstraintCatalog, KeyConstraints, PrimaryKey};
///
/// let mut catalog = ConstraintCatalog::new();
/// assert!(catalog.is_empty());
///
/// let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
/// catalog.relvar_mut("USERS").keys = Some(KeyConstraints::new().with_primary_key(pk));
/// assert!(catalog.relvar("USERS").is_some());
///
/// catalog.remove_relvar("USERS");
/// assert!(catalog.is_empty());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConstraintCatalog {
    /// Per-relvar constraints, keyed by relvar name.
    #[serde(default)]
    relvars: BTreeMap<String, RelvarConstraints>,
    /// Database-wide constraints.
    #[serde(default, skip_serializing_if = "DatabaseConstraints::is_empty")]
    database: DatabaseConstraints,
}

impl ConstraintCatalog {
    /// Creates an empty catalog (a database with no constraints).
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a catalog from its two sections.
    ///
    /// A storage engine that keeps the sections apart on disk (for
    /// compatibility with catalogs written before database-wide
    /// constraints existed) uses this to put them back together.
    pub fn from_parts(
        relvars: BTreeMap<String, RelvarConstraints>,
        database: DatabaseConstraints,
    ) -> Self {
        Self { relvars, database }
    }

    /// The per-relvar section, keyed by relvar name.
    pub fn relvars(&self) -> &BTreeMap<String, RelvarConstraints> {
        &self.relvars
    }

    /// Returns `true` if the catalog records no constraints.
    pub fn is_empty(&self) -> bool {
        self.relvars.values().all(RelvarConstraints::is_empty) && self.database.is_empty()
    }

    /// The constraints recorded for `relvar`, if any.
    pub fn relvar(&self, relvar: &str) -> Option<&RelvarConstraints> {
        self.relvars.get(relvar)
    }

    /// The constraints recorded for `relvar`, inserting an empty entry if
    /// there is none.
    pub fn relvar_mut(&mut self, relvar: &str) -> &mut RelvarConstraints {
        self.relvars.entry(String::from(relvar)).or_default()
    }

    /// The database-wide constraints.
    pub fn database(&self) -> &DatabaseConstraints {
        &self.database
    }

    /// The database-wide constraints, mutably.
    pub fn database_mut(&mut self) -> &mut DatabaseConstraints {
        &mut self.database
    }

    /// Removes every constraint declared *on* `relvar`.
    ///
    /// Foreign keys of other relvars and database-wide constraints that
    /// refer to `relvar` are kept;
    /// [`Database::drop_relvar`](crate::database::Database::drop_relvar)
    /// refuses to drop a relvar that another constraint refers to.
    pub fn remove_relvar(&mut self, relvar: &str) {
        self.relvars.remove(relvar);
    }

    /// Iterates over `(relvar name, constraints)` pairs in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &RelvarConstraints)> {
        self.relvars
            .iter()
            .map(|(name, constraints)| (name.as_str(), constraints))
    }

    /// Every relvar name the catalog mentions: the relvars constraints are
    /// declared on, the relvars their foreign keys reference, and the
    /// relvars database-wide constraints refer to.
    ///
    /// A storage engine can use this to reject a catalog whose constraints
    /// name relvars that do not exist. Names may repeat.
    pub fn mentioned_relvars(&self) -> impl Iterator<Item = &str> {
        self.iter()
            .filter(|(_, constraints)| !constraints.is_empty())
            .flat_map(|(name, constraints)| {
                core::iter::once(name).chain(
                    constraints
                        .foreign_keys
                        .iter()
                        .flat_map(|fks| fks.foreign_keys())
                        .map(|fk| fk.referenced_relation_name()),
                )
            })
            .chain(self.database.referenced_relvars().map(|(relvar, _)| relvar))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::{ForeignKey, PrimaryKey};

    fn keyed() -> KeyConstraints {
        KeyConstraints::new().with_primary_key(PrimaryKey::new(vec!["id".into()]).unwrap())
    }

    #[test]
    fn empty_entries_do_not_count() {
        let mut catalog = ConstraintCatalog::new();
        catalog.relvar_mut("A");
        assert!(catalog.is_empty());
        assert_eq!(catalog.mentioned_relvars().count(), 0);
    }

    #[test]
    fn mentioned_relvars_include_foreign_key_targets() {
        let mut catalog = ConstraintCatalog::new();
        catalog.relvar_mut("PARENT").keys = Some(keyed());
        catalog.relvar_mut("CHILD").foreign_keys =
            Some(ForeignKeyConstraints::new().with_foreign_key(
                ForeignKey::new(vec!["pid".into()], "PARENT".into(), vec!["id".into()]).unwrap(),
            ));
        let mut names: Vec<&str> = catalog.mentioned_relvars().collect();
        names.sort_unstable();
        assert_eq!(names, ["CHILD", "PARENT", "PARENT"]);
    }

    #[test]
    fn mentioned_relvars_include_assertion_relvars() {
        use crate::constraints::{AssertionCondition, DeclarativeAssertion};
        use crate::query::Query;
        let mut catalog = ConstraintCatalog::new();
        catalog
            .database_mut()
            .assertions
            .push(DeclarativeAssertion::new(
                "disjoint",
                "",
                AssertionCondition::IsEmpty(Query::scan("A").intersect(Query::scan("B"))),
            ));
        assert!(!catalog.is_empty());
        let mut names: Vec<&str> = catalog.mentioned_relvars().collect();
        names.sort_unstable();
        assert_eq!(names, ["A", "B"]);
        assert_eq!(
            catalog.database().referenced_relvars().collect::<Vec<_>>(),
            [("A", "disjoint"), ("B", "disjoint")]
        );
        assert!(catalog.database().has_name("disjoint"));
    }

    #[test]
    fn round_trips_through_json() {
        let mut catalog = ConstraintCatalog::new();
        catalog.relvar_mut("A").keys = Some(keyed());
        let text = serde_json::to_string(&catalog).unwrap();
        let back: ConstraintCatalog = serde_json::from_str(&text).unwrap();
        assert!(
            back.relvar("A")
                .and_then(|c| c.keys.as_ref())
                .and_then(KeyConstraints::primary_key)
                .is_some()
        );
    }
}
