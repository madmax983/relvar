#![allow(dead_code)]
//! Relational Build System
//!
//! This module demonstrates how a Build System (like Make or Ninja) can be implemented
//! using purely relational algebra operations.
//!
//! # Concept
//!
//! - **Files**: Relation `(file: String, modified_at: Int)`.
//! - **Dependencies**: Relation `(target: String, source: String)`.
//!
//! The algorithm to find stale targets:
//! 1. Compute the transitive closure of the `dependencies` relation using `tclose`.
//! 2. Join the transitive closure with the `files` relation (as `target` and `source`) to get modified times.
//! 3. Restrict where `target.modified_at < source.modified_at`.
//! 4. Project out just the `target` column.

use relvar_core::{error::DatabaseError, values::Relation};

/// A Relational Build System.
///
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
/// use relvar::experimental::build_system::BuildSystem;
///
/// let file_type = TupleType::new()
///     .with_attribute("file", ScalarType::String)
///     .with_attribute("modified_at", ScalarType::Int);
/// let mut files = Relation::new(RelationType::new(file_type));
///
/// let dep_type = TupleType::new()
///     .with_attribute("target", ScalarType::String)
///     .with_attribute("source", ScalarType::String);
/// let mut deps = Relation::new(RelationType::new(dep_type));
///
/// let build_sys = BuildSystem::new(files, deps);
/// ```
pub struct BuildSystem {
    /// The files and their modification timestamps. Schema: (file: String, modified_at: Int)
    pub files: Relation,
    /// The dependency graph. Schema: (target: String, source: String)
    pub dependencies: Relation,
}

impl BuildSystem {
    /// Creates a new BuildSystem by initializing the dependency graph and file states.
    ///
    /// The `files` relation should track file modification timestamps, while the
    /// `dependencies` relation forms a directed graph between build targets and their sources.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
    /// use relvar::experimental::build_system::BuildSystem;
    ///
    /// let file_type = TupleType::new()
    ///     .with_attribute("file", ScalarType::String)
    ///     .with_attribute("modified_at", ScalarType::Int);
    /// let files = Relation::new(RelationType::new(file_type));
    ///
    /// let dep_type = TupleType::new()
    ///     .with_attribute("target", ScalarType::String)
    ///     .with_attribute("source", ScalarType::String);
    /// let deps = Relation::new(RelationType::new(dep_type));
    ///
    /// let build_sys = BuildSystem::new(files, deps);
    /// ```
    pub fn new(files: Relation, dependencies: Relation) -> Self {
        Self {
            files,
            dependencies,
        }
    }

    /// Computes the set of targets that need to be rebuilt.
    ///
    /// This mimics a `make` utility's core logic using relational algebra. It first
    /// finds all direct and indirect dependencies by calculating the transitive
    /// closure of the graph. It then joins these edges against the file states
    /// and flags any targets where a transitive source is newer than the target itself.
    ///
    /// Returns a [`Relation`] with a single attribute `target` containing the stale files,
    /// or a [`DatabaseError`] if the algebraic operations fail.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
    /// use relvar::experimental::build_system::BuildSystem;
    ///
    /// let file_type = TupleType::new()
    ///     .with_attribute("file", ScalarType::String)
    ///     .with_attribute("modified_at", ScalarType::Int);
    /// let mut files = Relation::new(RelationType::new(file_type));
    /// files.insert(tuple! { file: "main.c", modified_at: 100i64 }).unwrap();
    /// files.insert(tuple! { file: "main.o", modified_at: 50i64 }).unwrap();
    ///
    /// let dep_type = TupleType::new()
    ///     .with_attribute("target", ScalarType::String)
    ///     .with_attribute("source", ScalarType::String);
    /// let mut deps = Relation::new(RelationType::new(dep_type));
    /// deps.insert(tuple! { target: "main.o", source: "main.c" }).unwrap();
    ///
    /// let build_sys = BuildSystem::new(files, deps);
    /// let stale = build_sys.compute_stale_targets().unwrap();
    ///
    /// assert_eq!(stale.cardinality(), 1);
    /// ```
    pub fn compute_stale_targets(&self) -> Result<Relation, DatabaseError> {
        // 1. Get the transitive closure of dependencies
        // tclose("target", "source") gives all direct and indirect dependencies.
        // It returns a relation with the same schema as dependencies: (target: String, source: String)
        let closure = self.dependencies.tclose("target", "source")?;

        // 2. Join with files to get the target's modified_at
        // Rename 'files' schema from (file, modified_at) to (target, target_modified_at)
        let target_files = self
            .files
            .rename(&[("file", "target")])
            .rename(&[("modified_at", "target_modified_at")]);

        let with_target_time = closure.join(&target_files)?;

        // 3. Join with files to get the source's modified_at
        // Rename 'files' schema from (file, modified_at) to (source, source_modified_at)
        let source_files = self
            .files
            .rename(&[("file", "source")])
            .rename(&[("modified_at", "source_modified_at")]);

        let with_both_times = with_target_time.join(&source_files)?;

        // 4. Restrict to cases where target_modified_at < source_modified_at
        let stale_edges = with_both_times.restrict(|t| {
            let target_time = t.get_typed::<i64>("target_modified_at").unwrap_or(0);
            let source_time = t.get_typed::<i64>("source_modified_at").unwrap_or(0);
            target_time < source_time
        });

        // 5. Project just the target
        let stale_targets = stale_edges.project(&["target"]);

        Ok(stale_targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    fn setup_relations() -> (Relation, Relation) {
        let file_type = TupleType::new()
            .with_attribute("file", ScalarType::String)
            .with_attribute("modified_at", ScalarType::Int);
        let files = Relation::new(RelationType::new(file_type));

        let dep_type = TupleType::new()
            .with_attribute("target", ScalarType::String)
            .with_attribute("source", ScalarType::String);
        let deps = Relation::new(RelationType::new(dep_type));

        (files, deps)
    }

    #[test]
    fn test_build_system_stale_direct_dependency() {
        let (mut files, mut deps) = setup_relations();

        // main.o depends on main.c
        deps.insert(tuple! { target: "main.o", source: "main.c" })
            .unwrap();

        // main.c is newer than main.o
        files
            .insert(tuple! { file: "main.c", modified_at: 100i64 })
            .unwrap();
        files
            .insert(tuple! { file: "main.o", modified_at: 50i64 })
            .unwrap();

        let build_sys = BuildSystem::new(files, deps);
        let stale = build_sys.compute_stale_targets().unwrap();

        assert_eq!(stale.cardinality(), 1);
        let mut found = false;
        for t in stale.tuples() {
            if t.get_typed::<String>("target").unwrap() == "main.o" {
                found = true;
            }
        }
        assert!(found, "main.o should be stale");
    }

    #[test]
    fn test_build_system_up_to_date() {
        let (mut files, mut deps) = setup_relations();

        // main.o depends on main.c
        deps.insert(tuple! { target: "main.o", source: "main.c" })
            .unwrap();

        // main.o is newer than main.c
        files
            .insert(tuple! { file: "main.c", modified_at: 50i64 })
            .unwrap();
        files
            .insert(tuple! { file: "main.o", modified_at: 100i64 })
            .unwrap();

        let build_sys = BuildSystem::new(files, deps);
        let stale = build_sys.compute_stale_targets().unwrap();

        assert_eq!(stale.cardinality(), 0);
    }

    #[test]
    fn test_build_system_transitive_dependency() {
        let (mut files, mut deps) = setup_relations();

        // app depends on main.o
        deps.insert(tuple! { target: "app", source: "main.o" })
            .unwrap();
        // main.o depends on main.c
        deps.insert(tuple! { target: "main.o", source: "main.c" })
            .unwrap();
        // main.c depends on utils.h
        deps.insert(tuple! { target: "main.c", source: "utils.h" })
            .unwrap();

        // utils.h was just modified! Everything else is older.
        files
            .insert(tuple! { file: "utils.h", modified_at: 200i64 })
            .unwrap();
        files
            .insert(tuple! { file: "main.c", modified_at: 50i64 })
            .unwrap();
        files
            .insert(tuple! { file: "main.o", modified_at: 60i64 })
            .unwrap();
        files
            .insert(tuple! { file: "app", modified_at: 70i64 })
            .unwrap();

        let build_sys = BuildSystem::new(files, deps);
        let stale = build_sys.compute_stale_targets().unwrap();

        // All of app, main.o, and main.c should be stale because utils.h is newer
        assert_eq!(stale.cardinality(), 3);
        let mut stale_list = Vec::new();
        for t in stale.tuples() {
            stale_list.push(t.get_typed::<String>("target").unwrap());
        }
        stale_list.sort();
        assert_eq!(stale_list, vec!["app", "main.c", "main.o"]);
    }
}
