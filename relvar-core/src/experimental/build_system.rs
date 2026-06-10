//! Relational Build System
//!
//! Models a Build System (like Make/Ninja) using pure relational algebra.
//! Dependencies are an edges relation, and file timestamps are a relation.
//! Stale targets are resolved by finding files older than their dependencies
//! via transitive closure (`tclose`), relational joins, and aggregations.

use crate::algebra::Aggregation;
use crate::error::DatabaseError;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A relational build system representation.
pub struct BuildSystem {
    files: Relation,
    dependencies: Relation,
}

impl Default for BuildSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildSystem {
    /// Creates a new Build System model.
    pub fn new() -> Self {
        let files_heading = TupleType::new()
            .with_attribute("name", ScalarType::String)
            .with_attribute("timestamp", ScalarType::Int);
        let files = Relation::new(RelationType::new(files_heading));

        let deps_heading = TupleType::new()
            .with_attribute("target", ScalarType::String)
            .with_attribute("dependency", ScalarType::String);
        let dependencies = Relation::new(RelationType::new(deps_heading));

        Self {
            files,
            dependencies,
        }
    }

    /// Adds a file and its last modified timestamp to the build system.
    pub fn add_file(&mut self, name: &str, timestamp: i64) -> Result<(), DatabaseError> {
        self.files
            .insert(tuple! {
                name: name.to_string(),
                timestamp: timestamp
            })
            .map_err(DatabaseError::Relation)?;
        Ok(())
    }

    /// Adds a dependency edge between a target and its prerequisite.
    pub fn add_dependency(&mut self, target: &str, dependency: &str) -> Result<(), DatabaseError> {
        self.dependencies
            .insert(tuple! {
                target: target.to_string(),
                dependency: dependency.to_string()
            })
            .map_err(DatabaseError::Relation)?;
        Ok(())
    }

    /// Computes and returns the relation of stale targets.
    ///
    /// A target is considered stale if its timestamp is older than the max
    /// timestamp of any of its dependencies (direct or indirect).
    pub fn get_stale_targets(&self) -> Result<Relation, DatabaseError> {
        // 1. Transitive closure of dependencies to find all indirect dependencies
        let all_deps = self.dependencies.tclose("target", "dependency")?;

        // 2. Join with files to get dependency timestamps
        let dep_files = self
            .files
            .rename(&[("name", "dependency"), ("timestamp", "dep_timestamp")]);
        let deps_with_times = all_deps.join(&dep_files)?;

        // 3. Aggregate max timestamp per target
        let max_dep_times = deps_with_times
            .summarize(
                &["target"],
                &[Aggregation::max(
                    "max_dep_time",
                    "dep_timestamp",
                    ScalarType::Int,
                )],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Join with target timestamps
        let target_files = self
            .files
            .rename(&[("name", "target"), ("timestamp", "target_timestamp")]);
        let targets_with_max_deps = max_dep_times.join(&target_files)?;

        // 5. Restrict to those where target_timestamp < max_dep_time
        let stale = targets_with_max_deps.restrict(|t: &crate::values::Tuple| {
            let target_time = t.get_typed::<i64>("target_timestamp").unwrap_or(0);
            let max_dep_time = t.get_typed::<i64>("max_dep_time").unwrap_or(0);
            target_time < max_dep_time
        });

        // 6. Project target names
        Ok(stale.project(&["target"]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_system_stale_targets() {
        let mut build = BuildSystem::new();
        build.add_file("main.c", 100).unwrap();
        build.add_file("utils.c", 100).unwrap();
        build.add_file("utils.h", 150).unwrap(); // utils.h is newer
        build.add_file("main.o", 110).unwrap(); // built after main.c but before utils.h was updated
        build.add_file("utils.o", 110).unwrap(); // built before utils.h
        build.add_file("program", 120).unwrap(); // built after .o files

        build.add_dependency("main.o", "main.c").unwrap();
        build.add_dependency("main.o", "utils.h").unwrap();
        build.add_dependency("utils.o", "utils.c").unwrap();
        build.add_dependency("utils.o", "utils.h").unwrap();
        build.add_dependency("program", "main.o").unwrap();
        build.add_dependency("program", "utils.o").unwrap();

        let stale = build.get_stale_targets().unwrap();
        assert_eq!(stale.cardinality(), 3);

        // Expected stale: main.o, utils.o, program
        assert!(stale.contains(&tuple! { target: "main.o".to_string() }));
        assert!(stale.contains(&tuple! { target: "utils.o".to_string() }));
        assert!(stale.contains(&tuple! { target: "program".to_string() }));
    }
}
