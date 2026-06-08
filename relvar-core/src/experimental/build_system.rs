use crate::error::DatabaseError;
use crate::values::Relation;

/// A Relational Build System.
///
/// Resolves stale targets (like Make or Ninja) using pure relational algebra.
pub struct BuildSystem;

impl BuildSystem {
    /// Identifies all targets that need to be rebuilt.
    ///
    /// A target needs to be rebuilt if:
    /// 1. It is missing from the `files` relation.
    /// 2. It depends (transitively) on a source that is missing.
    /// 3. It depends (transitively) on a source that is newer than the target.
    ///
    /// # Arguments
    /// * `files` - A relation with heading `(name: String, timestamp: Int)`.
    /// * `deps` - A relation with heading `(target: String, source: String)`.
    ///
    /// # Returns
    /// A relation with heading `(target: String)` containing all stale targets.
    pub fn get_stale_targets(files: &Relation, deps: &Relation) -> Result<Relation, DatabaseError> {
        // Compute all transitive dependencies: (target, source)
        let uniform_deps = deps.rename(&[("target", "node1"), ("source", "node2")]);
        let all_deps_uniform = uniform_deps.tclose("node1", "node2")?;
        let all_deps = all_deps_uniform.rename(&[("node1", "target"), ("node2", "source")]);

        // 1. Missing targets: targets that appear in `deps` but not in `files`.
        let all_targets = deps.project(&["target"]);
        let existing_files_as_targets = files.rename(&[("name", "target")]).project(&["target"]);
        let missing_targets = all_targets
            .difference(&existing_files_as_targets)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 2. Targets with missing sources: targets that depend on a source that is not in `files`.
        let all_sources = all_deps.project(&["source"]);
        let existing_files_as_sources = files.rename(&[("name", "source")]).project(&["source"]);
        let missing_sources = all_sources
            .difference(&existing_files_as_sources)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let targets_with_missing_sources = all_deps.join(&missing_sources)?.project(&["target"]);

        // 3. Stale due to time: target exists, source exists, and source is newer than target.
        let source_ts = files.rename(&[("name", "source"), ("timestamp", "source_ts")]);
        let target_ts = files.rename(&[("name", "target"), ("timestamp", "target_ts")]);

        let target_source_times = all_deps.join(&source_ts)?.join(&target_ts)?;
        let stale_due_to_time = target_source_times
            .restrict(|t| {
                let s_ts = t.get_typed::<i64>("source_ts").unwrap_or(0);
                let t_ts = t.get_typed::<i64>("target_ts").unwrap_or(0);
                s_ts > t_ts
            })
            .project(&["target"]);

        // Combine all stale conditions
        let combined = missing_targets
            .union(&targets_with_missing_sources)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .union(&stale_due_to_time)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        Ok(combined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    fn setup_relations() -> (Relation, Relation) {
        let files_heading = TupleType::new()
            .with_attribute("name", ScalarType::String)
            .with_attribute("timestamp", ScalarType::Int);
        let files = Relation::new(RelationType::new(files_heading));

        let deps_heading = TupleType::new()
            .with_attribute("target", ScalarType::String)
            .with_attribute("source", ScalarType::String);
        let deps = Relation::new(RelationType::new(deps_heading));

        (files, deps)
    }

    #[test]
    fn test_stale_due_to_time() {
        let (mut files, mut deps) = setup_relations();

        // C depends on B, B depends on A
        deps.insert(tuple! { target: "C", source: "B" }).unwrap();
        deps.insert(tuple! { target: "B", source: "A" }).unwrap();

        // A is newest (100), B is older (50), C is oldest (10)
        files
            .insert(tuple! { name: "A", timestamp: 100i64 })
            .unwrap();
        files
            .insert(tuple! { name: "B", timestamp: 50i64 })
            .unwrap();
        files
            .insert(tuple! { name: "C", timestamp: 10i64 })
            .unwrap();

        let stale = BuildSystem::get_stale_targets(&files, &deps).unwrap();

        // B is stale because A is newer (100 > 50)
        // C is stale because B is newer (50 > 10) AND A is newer (100 > 10)
        assert_eq!(stale.cardinality(), 2);
        assert!(stale.contains(&tuple! { target: "B" }));
        assert!(stale.contains(&tuple! { target: "C" }));
    }

    #[test]
    fn test_missing_target() {
        let (mut files, mut deps) = setup_relations();

        deps.insert(tuple! { target: "main.o", source: "main.c" })
            .unwrap();

        // main.c exists, main.o is missing
        files
            .insert(tuple! { name: "main.c", timestamp: 100i64 })
            .unwrap();

        let stale = BuildSystem::get_stale_targets(&files, &deps).unwrap();

        assert_eq!(stale.cardinality(), 1);
        assert!(stale.contains(&tuple! { target: "main.o" }));
    }

    #[test]
    fn test_missing_source() {
        let (mut files, mut deps) = setup_relations();

        deps.insert(tuple! { target: "main.o", source: "main.c" })
            .unwrap();

        // main.o exists, but main.c is missing
        files
            .insert(tuple! { name: "main.o", timestamp: 100i64 })
            .unwrap();

        let stale = BuildSystem::get_stale_targets(&files, &deps).unwrap();

        assert_eq!(stale.cardinality(), 1);
        assert!(stale.contains(&tuple! { target: "main.o" }));
    }

    #[test]
    fn test_up_to_date() {
        let (mut files, mut deps) = setup_relations();

        deps.insert(tuple! { target: "main.o", source: "main.c" })
            .unwrap();

        // main.o (100) is newer than main.c (50)
        files
            .insert(tuple! { name: "main.c", timestamp: 50i64 })
            .unwrap();
        files
            .insert(tuple! { name: "main.o", timestamp: 100i64 })
            .unwrap();

        let stale = BuildSystem::get_stale_targets(&files, &deps).unwrap();

        assert_eq!(stale.cardinality(), 0);
    }
}
