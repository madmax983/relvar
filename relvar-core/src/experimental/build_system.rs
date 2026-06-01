use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A Relational Build System.
///
/// Models a build system (like Make or Ninja) using pure relational algebra.
/// Targets, sources, and their modification timestamps are modeled as relations.
/// Stale targets are resolved by finding files older than their dependencies
/// via transitive closure (`tclose`), relational joins, and restrictions.
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
    /// Creates a new BuildSystem.
    pub fn new() -> Self {
        let files_type = RelationType::new(
            TupleType::new()
                .with_attribute("file", ScalarType::String)
                .with_attribute("timestamp", ScalarType::Int),
        );

        let deps_type = RelationType::new(
            TupleType::new()
                .with_attribute("target", ScalarType::String)
                .with_attribute("source", ScalarType::String),
        );

        Self {
            files: Relation::new(files_type),
            dependencies: Relation::new(deps_type),
        }
    }

    /// Adds a file with its modification timestamp.
    pub fn add_file(&mut self, file: &str, timestamp: i64) -> Result<(), DatabaseError> {
        self.files.insert(crate::tuple! {
            file: file,
            timestamp: timestamp
        })?;
        Ok(())
    }

    /// Adds a dependency: `target` depends on `source`.
    pub fn add_dependency(&mut self, target: &str, source: &str) -> Result<(), DatabaseError> {
        self.dependencies.insert(crate::tuple! {
            target: target,
            source: source
        })?;
        Ok(())
    }

    /// Computes all targets that need to be rebuilt.
    ///
    /// A target is stale if its timestamp is strictly less than the timestamp
    /// of any of its transitive dependencies.
    pub fn get_stale_targets(&self) -> Result<Relation, DatabaseError> {
        // 1. Compute transitive closure of dependencies: closure(target, source)
        let closure = self.dependencies.tclose("target", "source")?;

        // 2. Join closure with files to get target timestamps
        let files_target = self
            .files
            .rename(&[("file", "target"), ("timestamp", "target_ts")]);

        let closure_with_target_ts = closure.join(&files_target)?;

        // 3. Join with files again to get source timestamps
        let files_source = self
            .files
            .rename(&[("file", "source"), ("timestamp", "source_ts")]);

        let full_info = closure_with_target_ts.join(&files_source)?;

        // 4. Restrict to cases where source_ts > target_ts
        let stale_paths = full_info.restrict(|t| {
            let source_ts = t.get_typed::<i64>("source_ts").unwrap_or(0);
            let target_ts = t.get_typed::<i64>("target_ts").unwrap_or(0);
            source_ts > target_ts
        });

        // 5. Project to get just the stale targets
        let stale_targets = stale_paths.project(&["target"]);

        Ok(stale_targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;

    #[test]
    fn test_build_system() {
        let mut build = BuildSystem::new();

        build.add_file("app", 100).unwrap();
        build.add_file("main.o", 90).unwrap();
        build.add_file("utils.o", 90).unwrap();
        build.add_file("main.c", 80).unwrap();
        build.add_file("utils.c", 80).unwrap();
        build.add_file("utils.h", 110).unwrap();

        build.add_dependency("app", "main.o").unwrap();
        build.add_dependency("app", "utils.o").unwrap();
        build.add_dependency("main.o", "main.c").unwrap();
        build.add_dependency("main.o", "utils.h").unwrap();
        build.add_dependency("utils.o", "utils.c").unwrap();
        build.add_dependency("utils.o", "utils.h").unwrap();

        let stale = build.get_stale_targets().unwrap();

        assert_eq!(stale.cardinality(), 3);
        assert!(stale.contains(&tuple! { target: "app" }));
        assert!(stale.contains(&tuple! { target: "main.o" }));
        assert!(stale.contains(&tuple! { target: "utils.o" }));
    }
}
