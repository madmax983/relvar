use crate::values::Relation;

/// A relational representation of a Build System (like Make or Ninja).
pub struct BuildSystem;

impl BuildSystem {
    /// Identifies targets that are stale given dependencies and file modification times.
    ///
    /// The `dependencies` relation must have `target` (String) and `dependency` (String).
    /// The `file_times` relation must have `file` (String) and `mtime` (Int).
    pub fn find_stale_targets(dependencies: &Relation, file_times: &Relation) -> Relation {
        // 1. Find all transitive dependencies
        let closure = dependencies.clone().tclose("target", "dependency").unwrap();

        // 2. We need to compare target's mtime and dependency's mtime
        let target_times = file_times
            .clone()
            .rename(&[("file", "target"), ("mtime", "target_mtime")]);
        let dep_times = file_times
            .clone()
            .rename(&[("file", "dependency"), ("mtime", "dep_mtime")]);

        let all_targets = dependencies.clone().project(&["target"]);
        let existing_targets = file_times
            .clone()
            .rename(&[("file", "target")])
            .project(&["target"]);

        // Targets missing from file_times
        let missing_targets = all_targets.difference(&existing_targets).unwrap();

        // Join closure with target_times and dep_times
        let joined = closure
            .join(&target_times)
            .unwrap()
            .join(&dep_times)
            .unwrap();

        // Restrict where dep_mtime > target_mtime
        let outdated = joined
            .restrict(|t| {
                let target_mtime = t.get_typed::<i64>("target_mtime").unwrap_or(0);
                let dep_mtime = t.get_typed::<i64>("dep_mtime").unwrap_or(0);
                dep_mtime > target_mtime
            })
            .project(&["target"]);

        missing_targets.union(&outdated).unwrap()
    }
}
