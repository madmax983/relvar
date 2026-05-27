use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A Build System (like Make) modeled purely using relational algebra.
///
/// Targets, dependencies, and file modified times are stored as relations.
/// Stale targets are discovered by computing the transitive closure of dependencies,
/// joining with file modified times, and filtering where the target's time is
/// older than any of its transitive dependencies.
pub struct BuildSystem {
    /// A relation of `(target, dependency)`
    pub dependencies: Relation,
    /// A relation of `(name, modified_time)`
    pub files: Relation,
}

impl BuildSystem {
    /// Creates a new, empty relational build system.
    pub fn new() -> Self {
        let dep_type = RelationType::new(
            TupleType::new()
                .with_attribute("target", ScalarType::String)
                .with_attribute("dependency", ScalarType::String),
        );
        let file_type = RelationType::new(
            TupleType::new()
                .with_attribute("name", ScalarType::String)
                .with_attribute("modified_time", ScalarType::Int),
        );
        Self {
            dependencies: Relation::new(dep_type),
            files: Relation::new(file_type),
        }
    }

    /// Adds a dependency rule: `target` depends on `dependency`.
    pub fn add_dependency(&mut self, target: &str, dependency: &str) -> Result<(), DatabaseError> {
        let t = crate::tuple! {
            target: target,
            dependency: dependency
        };
        self.dependencies
            .insert(t)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Sets the modified time for a file.
    pub fn set_file_time(&mut self, name: &str, modified_time: i64) -> Result<(), DatabaseError> {
        // Delete the old timestamp if it exists, otherwise it will have multiple timestamps and act strangely.
        let old = self
            .files
            .restrict(move |t| t.get_typed::<String>("name") == Some(name.to_string()));
        let new_files = self.files.clone();
        self.files = new_files
            .difference_into(&old)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let t = crate::tuple! {
            name: name,
            modified_time: modified_time
        };
        self.files
            .insert(t)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(())
    }

    /// Finds targets that exist but are older than one or more of their (transitive) dependencies.
    pub fn get_stale_targets(&self) -> Result<Relation, DatabaseError> {
        let mut all_deps = self.dependencies.clone();
        if self.dependencies.cardinality() > 0 {
            let tc = self.dependencies.tclose("target", "dependency");
            if let Ok(tc) = tc {
                all_deps = tc;
            }
        }

        let files_as_dep = self
            .files
            .rename(&[("name", "dependency"), ("modified_time", "dep_time")]);
        let deps_with_times = all_deps.join(&files_as_dep)?;

        let files_as_target = self
            .files
            .rename(&[("name", "target"), ("modified_time", "target_time")]);
        let targets_with_times = deps_with_times.join(&files_as_target)?;

        let stale = targets_with_times.restrict(|t| {
            let target_time = t.get_typed::<i64>("target_time").unwrap_or(0);
            let dep_time = t.get_typed::<i64>("dep_time").unwrap_or(0);
            target_time < dep_time
        });

        Ok(stale.project(&["target"]))
    }

    /// Finds targets that are declared in dependencies but do not exist in the files relation.
    pub fn get_missing_targets(&self) -> Result<Relation, DatabaseError> {
        let all_targets = self.dependencies.project(&["target"]);
        let existing_files = self
            .files
            .rename(&[("name", "target")])
            .project(&["target"]);
        all_targets
            .difference_into(&existing_files)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    /// Finds all targets that need to be rebuilt (either missing or stale).
    pub fn get_targets_to_rebuild(&self) -> Result<Relation, DatabaseError> {
        let stale = self.get_stale_targets()?;
        let missing = self.get_missing_targets()?;
        let old_stale = stale.clone();
        old_stale
            .union_into(&missing)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }
}

impl Default for BuildSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_system() {
        let mut build = BuildSystem::new();

        // main.o depends on main.c and lib.h
        build.add_dependency("main.o", "main.c").unwrap();
        build.add_dependency("main.o", "lib.h").unwrap();
        // app depends on main.o
        build.add_dependency("app", "main.o").unwrap();

        // Initially nothing exists, so targets are missing
        let to_rebuild = build.get_targets_to_rebuild().unwrap();
        assert_eq!(to_rebuild.cardinality(), 2); // main.o and app
        assert!(to_rebuild.contains(&crate::tuple! { target: "main.o" }));
        assert!(to_rebuild.contains(&crate::tuple! { target: "app" }));

        // Now files exist
        build.set_file_time("main.c", 100).unwrap();
        build.set_file_time("lib.h", 100).unwrap();
        build.set_file_time("main.o", 150).unwrap();
        build.set_file_time("app", 200).unwrap();

        let to_rebuild = build.get_targets_to_rebuild().unwrap();
        assert_eq!(to_rebuild.cardinality(), 0); // Nothing to rebuild

        // Update a dependency
        build.set_file_time("lib.h", 250).unwrap(); // lib.h is newer than main.o and app

        let to_rebuild = build.get_targets_to_rebuild().unwrap();
        assert_eq!(to_rebuild.cardinality(), 2); // main.o and app are stale
        assert!(to_rebuild.contains(&crate::tuple! { target: "main.o" }));
        assert!(to_rebuild.contains(&crate::tuple! { target: "app" }));
    }
}
