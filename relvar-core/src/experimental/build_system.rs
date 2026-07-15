//! Relational Build System
//!
//! Models a Build System (like Make/Ninja) using pure relational algebra.
//! Dependencies are an edges relation, and file timestamps are a relation.
//! Stale targets are resolved by finding files older than their dependencies
//! via transitive closure (`tclose`), relational joins, and restrictions.

use crate::error::DatabaseError;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A relational build system representation.
#[derive(Debug, Clone)]
pub struct BuildSystem {
    /// Relation of files: (name: String, mtime: Int)
    pub files: Relation,
    /// Relation of dependencies: (target: String, source: String)
    pub dependencies: Relation,
}

impl Default for BuildSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildSystem {
    /// Creates a new, empty relational build system.
    pub fn new() -> Self {
        let files_type = RelationType::new(
            TupleType::new()
                .with_attribute("name", ScalarType::String)
                .with_attribute("mtime", ScalarType::Int),
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

    /// Computes and returns a relation of all stale targets.
    /// A target is stale if:
    /// 1. It is missing from the files relation.
    /// 2. It transitively depends on a source file that has a strictly greater modification time.
    ///
    /// The returned relation has heading (target: String).
    pub fn get_stale_targets(&self) -> Result<Relation, DatabaseError> {
        // 1. Compute all transitive dependencies (target, source)
        // tclose includes both direct and transitive edges.
        let all_deps = self.dependencies.tclose("target", "source")?;

        // 2. Find dependencies where target is older than source
        // rename files(name, mtime) -> (source, source_mtime)
        let source_files = self
            .files
            .rename(&[("name", "source"), ("mtime", "source_mtime")]);
        let deps_with_source_time = all_deps.join(&source_files)?;

        // rename files(name, mtime) -> (target, target_mtime)
        let target_files = self
            .files
            .rename(&[("name", "target"), ("mtime", "target_mtime")]);
        let deps_with_both_times = deps_with_source_time.join(&target_files)?;

        // Restrict where source_mtime > target_mtime
        let stale_due_to_time = deps_with_both_times.restrict(|t| {
            let s_time = t.get_typed::<i64>("source_mtime").unwrap_or(0);
            let t_time = t.get_typed::<i64>("target_mtime").unwrap_or(0);
            s_time > t_time
        });

        let stale_from_time = stale_due_to_time.project(&["target"]);

        // 3. Find targets that have no mtime (missing)
        let all_targets = self.dependencies.project(&["target"]);
        let known_files = self.files.project(&["name"]).rename(&[("name", "target")]);
        let missing_targets = all_targets
            .difference(&known_files)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Union them together
        stale_from_time
            .union(&missing_targets)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;

    #[test]
    fn test_build_system_stale_resolution() {
        let mut sys = BuildSystem::new();

        // Dependency graph:
        // header.h (200) -> util.c (40) -> util.o (45) -> app (60)
        // source.c (100) -> source.o (50) -> app (60)

        sys.files
            .insert(tuple! { name: "source.c", mtime: 100i64 })
            .unwrap();
        sys.files
            .insert(tuple! { name: "source.o", mtime: 50i64 })
            .unwrap();
        sys.files
            .insert(tuple! { name: "app", mtime: 60i64 })
            .unwrap();
        sys.files
            .insert(tuple! { name: "util.c", mtime: 40i64 })
            .unwrap();
        sys.files
            .insert(tuple! { name: "util.o", mtime: 45i64 })
            .unwrap();
        sys.files
            .insert(tuple! { name: "header.h", mtime: 200i64 })
            .unwrap();

        sys.dependencies
            .insert(tuple! { target: "app", source: "source.o" })
            .unwrap();
        sys.dependencies
            .insert(tuple! { target: "app", source: "util.o" })
            .unwrap();
        sys.dependencies
            .insert(tuple! { target: "source.o", source: "source.c" })
            .unwrap();
        sys.dependencies
            .insert(tuple! { target: "util.o", source: "util.c" })
            .unwrap();
        sys.dependencies
            .insert(tuple! { target: "util.c", source: "header.h" })
            .unwrap();

        // Add a missing target
        sys.dependencies
            .insert(tuple! { target: "missing.o", source: "source.c" })
            .unwrap();

        let stale = sys.get_stale_targets().unwrap();

        // Expect stale: "app", "source.o", "util.c", "util.o", "missing.o"
        assert_eq!(stale.cardinality(), 5);
        assert!(stale.contains(&tuple! { target: "app" }));
        assert!(stale.contains(&tuple! { target: "source.o" }));
        assert!(stale.contains(&tuple! { target: "util.c" }));
        assert!(stale.contains(&tuple! { target: "util.o" }));
        assert!(stale.contains(&tuple! { target: "missing.o" }));
    }
}
