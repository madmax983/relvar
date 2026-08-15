//! A relational approach to Build Systems (like Make or Ninja).
//!
//! This module provides a minimal, experimental implementation of a Build System
//! built entirely on top of relational algebra. Instead of using graph traversal
//! or topological sorts to resolve dependencies, it stores target-dependency relationships
//! and file timestamps as relations. Stale targets are found declaratively using
//! joins, transitive closures (`tclose`), and set differences.

use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, Tuple};

/// A Build System modeled purely using relational algebra.
pub struct BuildSystem {
    /// Relation of (target: String, dependency: String)
    pub dependencies: Relation,
    /// Relation of (file: String, modified_time: Int)
    pub file_stats: Relation,
}

impl Default for BuildSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildSystem {
    /// Creates a new, empty BuildSystem.
    pub fn new() -> Self {
        let dep_type = RelationType::new(
            TupleType::new()
                .with_attribute("target", ScalarType::String)
                .with_attribute("dependency", ScalarType::String),
        );
        let dependencies = Relation::new(dep_type);

        let stat_type = RelationType::new(
            TupleType::new()
                .with_attribute("file", ScalarType::String)
                .with_attribute("modified_time", ScalarType::Int),
        );
        let file_stats = Relation::new(stat_type);

        Self {
            dependencies,
            file_stats,
        }
    }

    /// Adds a dependency rule: `target` depends on `dependency`.
    pub fn add_dependency(&mut self, target: &str, dependency: &str) -> Result<(), DatabaseError> {
        use crate::tuple;
        let t = tuple! {
            target: target,
            dependency: dependency,
        };
        self.dependencies
            .insert(t)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Updates the modified time of a file.
    pub fn update_stat(&mut self, file: &str, modified_time: i64) -> Result<(), DatabaseError> {
        use crate::tuple;
        let t = tuple! {
            file: file,
            modified_time: modified_time,
        };

        let to_remove = self
            .file_stats
            .clone()
            .restrict_into(|tup: &Tuple| tup.get_typed::<String>("file").unwrap() == file);

        self.file_stats = self
            .file_stats
            .difference(&to_remove)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        self.file_stats
            .insert(t)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Returns a relation of targets that need to be rebuilt.
    ///
    /// A target is stale if:
    /// 1. It does not exist in `file_stats` (missing).
    /// 2. It has a `file_stat` but a transitive dependency has a newer `file_stat`.
    pub fn get_stale_targets(&self) -> Result<Relation, DatabaseError> {
        // 1. Transitive closure of dependencies
        let all_deps = self
            .dependencies
            .tclose("target", "dependency")
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Join with file_stats to get dependency times
        let dep_times = self
            .file_stats
            .rename(&[("file", "dependency")])
            .rename(&[("modified_time", "dep_time")]);

        let deps_with_times = all_deps
            .join(&dep_times)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Get target times
        let target_times = self
            .file_stats
            .rename(&[("file", "target")])
            .rename(&[("modified_time", "target_time")]);

        // Condition 1: Target has no time (it's missing)
        let all_targets = self.dependencies.project(&["target"]);
        let existing_targets = target_times.project(&["target"]);
        let missing_targets = all_targets
            .difference(&existing_targets)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Condition 2: Target is older than dependency
        let full_info = deps_with_times
            .join(&target_times)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let outdated_targets_full = full_info.clone().restrict_into(|t: &Tuple| {
            let target_time = t.get_typed::<i64>("target_time").unwrap();
            let dep_time = t.get_typed::<i64>("dep_time").unwrap();
            target_time < dep_time
        });

        let outdated_targets = outdated_targets_full.project(&["target"]);

        // Union missing and outdated targets
        let stale = missing_targets
            .union(&outdated_targets)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(stale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_system_stale_targets() {
        let mut build = BuildSystem::new();

        build.add_dependency("app", "main.o").unwrap();
        build.add_dependency("app", "lib.o").unwrap();
        build.add_dependency("main.o", "main.c").unwrap();
        build.add_dependency("main.o", "header.h").unwrap();
        build.add_dependency("lib.o", "lib.c").unwrap();
        build.add_dependency("lib.o", "header.h").unwrap();

        let stale = build.get_stale_targets().unwrap();
        assert_eq!(stale.cardinality(), 3);

        build.update_stat("header.h", 10).unwrap();
        build.update_stat("main.c", 10).unwrap();
        build.update_stat("lib.c", 10).unwrap();

        build.update_stat("main.o", 20).unwrap();
        build.update_stat("lib.o", 20).unwrap();

        build.update_stat("app", 30).unwrap();

        let stale = build.get_stale_targets().unwrap();
        assert_eq!(stale.cardinality(), 0);

        build.update_stat("header.h", 40).unwrap();

        let stale = build.get_stale_targets().unwrap();
        assert_eq!(stale.cardinality(), 3);

        let is_stale1 = |name: &str| -> bool {
            let tuples: Vec<&Tuple> = stale.tuples().collect();
            tuples
                .iter()
                .any(|t| t.get_typed::<String>("target").unwrap() == name)
        };

        assert!(is_stale1("app"));
        assert!(is_stale1("main.o"));
        assert!(is_stale1("lib.o"));

        build.update_stat("header.h", 10).unwrap();
        build.update_stat("main.c", 40).unwrap();

        let stale = build.get_stale_targets().unwrap();
        assert_eq!(
            stale.cardinality(),
            2,
            "Expected 2 stale targets, got {} {:?}",
            stale.cardinality(),
            stale
        );

        let is_stale2 = |name: &str| -> bool {
            let tuples: Vec<&Tuple> = stale.tuples().collect();
            tuples
                .iter()
                .any(|t| t.get_typed::<String>("target").unwrap() == name)
        };

        assert!(is_stale2("app"));
        assert!(is_stale2("main.o"));
        assert!(!is_stale2("lib.o"));
    }
}
