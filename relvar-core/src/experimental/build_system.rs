//! Relational Build System Simulator.
//!
//! Evaluates dependencies (e.g. from Make/Ninja) purely using relational algebra.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Finds targets that are out of date compared to their dependencies.
///
/// `dependencies` must have attributes: `target` (String), `dependency` (String).
/// `file_times` must have attributes: `file` (String), `timestamp` (Int).
pub fn find_stale_targets(
    dependencies: &Relation,
    file_times: &Relation,
) -> Result<Relation, DatabaseError> {
    let all_targets = dependencies.project(&["target"]);
    let all_files_as_targets = file_times.project(&["file"]).rename(&[("file", "target")]);
    let missing_targets = all_targets
        .difference(&all_files_as_targets)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let all_deps = dependencies.project(&["dependency"]);
    let all_files_as_deps = file_times
        .project(&["file"])
        .rename(&[("file", "dependency")]);
    let missing_deps = all_deps
        .difference(&all_files_as_deps)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let targets_with_missing_deps = dependencies.join(&missing_deps)?.project(&["target"]);

    let deps_with_time = dependencies
        .join(&file_times.rename(&[("file", "dependency"), ("timestamp", "dep_time")]))?;

    let targets_with_deps_and_times = deps_with_time
        .join(&file_times.rename(&[("file", "target"), ("timestamp", "target_time")]))?;

    let outdated_targets = targets_with_deps_and_times
        .restrict(|t| {
            let dep_time = t.get_typed::<i64>("dep_time").unwrap_or(0);
            let target_time = t.get_typed::<i64>("target_time").unwrap_or(0);
            dep_time > target_time
        })
        .project(&["target"]);

    let base_stale_1 = missing_targets
        .union(&targets_with_missing_deps)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
    let base_stale = base_stale_1
        .union(&outdated_targets)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let trans_deps = dependencies.tclose("target", "dependency")?;
    let base_stale_as_deps = base_stale.rename(&[("target", "dependency")]);

    let transitively_stale = trans_deps.join(&base_stale_as_deps)?.project(&["target"]);

    base_stale
        .union(&transitively_stale)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    fn make_deps() -> Relation {
        let heading = TupleType::new()
            .with_attribute("target", ScalarType::String)
            .with_attribute("dependency", ScalarType::String);
        let mut rel = Relation::new(RelationType::new(heading));
        rel.insert(tuple! {target: "app.exe", dependency: "main.o"})
            .unwrap();
        rel.insert(tuple! {target: "app.exe", dependency: "lib.o"})
            .unwrap();
        rel.insert(tuple! {target: "main.o", dependency: "main.c"})
            .unwrap();
        rel.insert(tuple! {target: "main.o", dependency: "header.h"})
            .unwrap();
        rel.insert(tuple! {target: "lib.o", dependency: "lib.c"})
            .unwrap();
        rel.insert(tuple! {target: "lib.o", dependency: "header.h"})
            .unwrap();
        rel
    }

    fn make_times(times: &[(&str, i64)]) -> Relation {
        let heading = TupleType::new()
            .with_attribute("file", ScalarType::String)
            .with_attribute("timestamp", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading));
        for &(file, ts) in times {
            rel.insert(tuple! {file: file, timestamp: ts}).unwrap();
        }
        rel
    }

    #[test]
    fn test_all_up_to_date() {
        let deps = make_deps();
        let times = make_times(&[
            ("header.h", 100),
            ("main.c", 100),
            ("lib.c", 100),
            ("main.o", 200),
            ("lib.o", 200),
            ("app.exe", 300),
        ]);

        let stale = find_stale_targets(&deps, &times).unwrap();
        assert_eq!(stale.cardinality(), 0);
    }

    #[test]
    fn test_missing_target() {
        let deps = make_deps();
        let times = make_times(&[
            ("header.h", 100),
            ("main.c", 100),
            ("lib.c", 100),
            ("main.o", 200),
            // lib.o is missing
            ("app.exe", 300),
        ]);

        let stale = find_stale_targets(&deps, &times).unwrap();
        // lib.o is missing so it's stale. app.exe depends on lib.o so it's also stale.
        assert_eq!(stale.cardinality(), 2);
        assert!(stale.contains(&tuple! {target: "lib.o"}));
        assert!(stale.contains(&tuple! {target: "app.exe"}));
    }

    #[test]
    fn test_outdated_dep() {
        let deps = make_deps();
        let times = make_times(&[
            ("header.h", 250), // Header was updated!
            ("main.c", 100),
            ("lib.c", 100),
            ("main.o", 200),
            ("lib.o", 200),
            ("app.exe", 300),
        ]);

        let stale = find_stale_targets(&deps, &times).unwrap();
        // header.h updated (250) > main.o (200), lib.o (200) -> both stale.
        // app.exe depends on stale main.o, lib.o -> stale.
        assert_eq!(stale.cardinality(), 3);
        assert!(stale.contains(&tuple! {target: "main.o"}));
        assert!(stale.contains(&tuple! {target: "lib.o"}));
        assert!(stale.contains(&tuple! {target: "app.exe"}));
    }
}
