//! Relational Build System.
//!
//! Modeled a Build System (like Make/Ninja) using pure relational algebra.
//! Dependencies are an edges relation, and file timestamps are a relation.
//! Stale targets are resolved by finding files older than their dependencies
//! via transitive closure (`tclose`), relational joins, and aggregations.

use crate::error::DatabaseError;
use crate::values::{Relation, Tuple};

/// Finds stale targets that need to be rebuilt.
///
/// `files` must be a relation with attributes `file` (String) and `timestamp` (Int).
/// `dependencies` must be a relation with attributes `target` (String) and `dependency` (String).
pub fn find_stale_targets(
    files: &Relation,
    dependencies: &Relation,
) -> Result<Relation, DatabaseError> {
    // Transitive closure of dependencies to get all direct and indirect dependencies for each target
    let closure = dependencies.tclose("target", "dependency")?;

    // Join with files to get target timestamps
    let files_as_target = files.clone().rename_into(&[("file", "target")]);
    let target_times = closure.join(&files_as_target)?;
    let target_times = target_times.rename_into(&[("timestamp", "target_timestamp")]);

    // Join with files again to get dependency timestamps
    let files_as_dep = files.clone().rename_into(&[("file", "dependency")]);
    let all_times = target_times.join(&files_as_dep)?;
    let all_times = all_times.rename_into(&[("timestamp", "dep_timestamp")]);

    // Restrict to pairs where target is older than dependency
    let stale_pairs = all_times.restrict(|t: &Tuple| {
        let t_time = t.get_typed::<i64>("target_timestamp").unwrap();
        let d_time = t.get_typed::<i64>("dep_timestamp").unwrap();
        t_time < d_time
    });

    // Project out only the stale targets
    Ok(stale_pairs.project_into(&["target"]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    fn make_files_relation(files_data: &[(&str, i64)]) -> Relation {
        let heading = TupleType::new()
            .with_attribute("file", ScalarType::String)
            .with_attribute("timestamp", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading));
        for &(file, timestamp) in files_data {
            rel.insert(tuple! {file: file.to_string(), timestamp: timestamp})
                .unwrap();
        }
        rel
    }

    fn make_dependencies_relation(deps_data: &[(&str, &str)]) -> Relation {
        let heading = TupleType::new()
            .with_attribute("target", ScalarType::String)
            .with_attribute("dependency", ScalarType::String);
        let mut rel = Relation::new(RelationType::new(heading));
        for &(target, dependency) in deps_data {
            rel.insert(tuple! {target: target.to_string(), dependency: dependency.to_string()})
                .unwrap();
        }
        rel
    }

    #[test]
    fn test_stale_targets() {
        // Files: main.c (ts=100), util.c (ts=100), util.h (ts=150), main.o (ts=120), util.o (ts=110), app (ts=130)
        let files = make_files_relation(&[
            ("main.c", 100),
            ("util.c", 100),
            ("util.h", 150), // Recently modified header
            ("main.o", 120),
            ("util.o", 110),
            ("app", 130),
        ]);

        let dependencies = make_dependencies_relation(&[
            ("app", "main.o"),
            ("app", "util.o"),
            ("main.o", "main.c"),
            ("main.o", "util.h"),
            ("util.o", "util.c"),
            ("util.o", "util.h"),
        ]);

        let stale = find_stale_targets(&files, &dependencies).unwrap();

        let mut targets: Vec<String> = stale
            .tuples()
            .map(|t| t.get_typed::<String>("target").unwrap())
            .collect();
        targets.sort();

        // util.h is newer (150) than main.o (120), util.o (110) and app (130)
        // so main.o, util.o and app should be stale
        assert_eq!(targets, vec!["app", "main.o", "util.o"]);
    }
}
