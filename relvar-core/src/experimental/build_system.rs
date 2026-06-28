//! Relational Build System
//!
//! This module models a build system (like Make or Ninja) using purely relational algebra.
//! The build graph and file metadata are represented as relations:
//! - `targets`: The files and their last-modified timestamps { `file`: String, `timestamp`: Int }.
//! - `dependencies`: The directed edges of the build graph { `target`: String, `depends_on`: String }.
//!
//! Finding "stale" targets that need to be rebuilt is typically done using recursive graph traversal.
//! Here, we use a purely declarative relational approach:
//! 1. Compute the transitive closure of dependencies (all indirect dependencies).
//! 2. Join the transitive dependencies with their target and dependency timestamps.
//! 3. Use relational restriction (`sigma`) to find any target that is older than any of its
//!    transitive dependencies.

use crate::error::DatabaseError;
use crate::values::Relation;

/// A purely relational build system stale-target detector.
pub struct RelationalBuildSystem;

impl RelationalBuildSystem {
    /// Identifies which targets need to be rebuilt because their dependencies (direct or indirect)
    /// are newer than the target itself.
    ///
    /// # Expected Schemas:
    /// - `targets`: { `file`: String, `timestamp`: Int }
    /// - `dependencies`: { `target`: String, `depends_on`: String }
    ///
    /// Returns a relation of { `file`: String } representing stale targets.
    pub fn find_stale_targets(
        targets: &Relation,
        dependencies: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // 1. Compute all transitive dependencies (target -> indirect_dependency)
        let all_deps = dependencies.tclose("target", "depends_on")?;

        // 2. Join with targets to get the timestamp of the target itself
        let target_timestamps = targets.rename(&[("file", "target"), ("timestamp", "target_ts")]);
        let deps_with_target_ts = all_deps.join(&target_timestamps)?;

        // 3. Join with targets again to get the timestamp of the dependency
        let dep_timestamps = targets.rename(&[("file", "depends_on"), ("timestamp", "dep_ts")]);
        let fully_joined = deps_with_target_ts.join(&dep_timestamps)?;

        // 4. Restrict (filter) to find where a dependency is newer than the target
        let stale = fully_joined.restrict(|t| {
            let target_ts = t.get_typed::<i64>("target_ts").unwrap_or(0);
            let dep_ts = t.get_typed::<i64>("dep_ts").unwrap_or(0);
            dep_ts > target_ts
        });

        // 5. Project out just the target names and rename back to `file`
        let result = stale.project(&["target"]).rename(&[("target", "file")]);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_find_stale_targets() {
        let targets_type = TupleType::new()
            .with_attribute("file", ScalarType::String)
            .with_attribute("timestamp", ScalarType::Int);

        let deps_type = TupleType::new()
            .with_attribute("target", ScalarType::String)
            .with_attribute("depends_on", ScalarType::String);

        let mut targets = Relation::new(RelationType::new(targets_type));
        let mut dependencies = Relation::new(RelationType::new(deps_type));

        // Target timestamps
        targets
            .insert(tuple! { file: "source.c", timestamp: 10i64 })
            .unwrap();
        targets
            .insert(tuple! { file: "header.h", timestamp: 20i64 })
            .unwrap();
        targets
            .insert(tuple! { file: "app.exe", timestamp: 15i64 })
            .unwrap();
        targets
            .insert(tuple! { file: "other.c", timestamp: 5i64 })
            .unwrap();
        targets
            .insert(tuple! { file: "other_lib.o", timestamp: 6i64 })
            .unwrap();

        // Dependencies
        dependencies
            .insert(tuple! { target: "app.exe", depends_on: "source.c" })
            .unwrap();
        dependencies
            .insert(tuple! { target: "source.c", depends_on: "header.h" })
            .unwrap();
        dependencies
            .insert(tuple! { target: "other_lib.o", depends_on: "other.c" })
            .unwrap();

        let stale = RelationalBuildSystem::find_stale_targets(&targets, &dependencies).unwrap();

        assert_eq!(stale.cardinality(), 2);
        assert!(stale.contains(&tuple! { file: "source.c" }));
        assert!(stale.contains(&tuple! { file: "app.exe" }));
    }
}
