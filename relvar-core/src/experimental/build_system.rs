//! A relational approach to Build Systems (like Make/Ninja).
//!
//! This module models a build system dependency graph using pure relational algebra.
//! It uses transitive closure to resolve dependency chains and relational joins/restrictions
//! to identify which targets are stale and need to be rebuilt based on file timestamps.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Resolves the set of "stale" targets that need to be rebuilt.
///
/// `files` relation must have attributes: `name` (String), `timestamp` (Int).
/// `dependencies` relation must have attributes: `target` (String), `source` (String).
///
/// A target needs rebuilding if:
/// 1. It is declared as a target in `dependencies` but does not exist in `files`.
/// 2. It exists in `files`, but at least one of its transitive dependencies exists
///    in `files` with a `timestamp` strictly greater than the target's `timestamp`.
pub fn get_stale_targets(
    files: &Relation,
    dependencies: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Compute transitive dependencies
    let trans_deps = dependencies.tclose("target", "source")?;

    // 2. Find missing targets (targets that don't exist in files)
    let all_targets = dependencies.project(&["target"]);
    let all_files_as_target = files.project(&["name"]).rename_into(&[("name", "target")]);
    let missing_targets = all_targets
        .difference(&all_files_as_target)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 3. Find out-of-date targets (target_time < source_time)
    // Join trans_deps with files to get source timestamps
    let deps_with_source_time = trans_deps
        .join(&files.rename(&[("name", "source"), ("timestamp", "source_time")]))
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Join with files again to get target timestamps
    let deps_with_both_times = deps_with_source_time
        .join(&files.rename(&[("name", "target"), ("timestamp", "target_time")]))
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Restrict where target_time < source_time
    let stale_existing = deps_with_both_times
        .restrict(|t| {
            let target_time = t.get_typed::<i64>("target_time").unwrap_or(0);
            let source_time = t.get_typed::<i64>("source_time").unwrap_or(0);
            target_time < source_time
        })
        .project(&["target"]);

    // Union missing and stale existing
    let final_stale = missing_targets
        .union(&stale_existing)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    Ok(final_stale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_build_system() {
        let files_type = RelationType::new(
            TupleType::new()
                .with_attribute("name".to_string(), ScalarType::String)
                .with_attribute("timestamp".to_string(), ScalarType::Int),
        );

        let deps_type = RelationType::new(
            TupleType::new()
                .with_attribute("target".to_string(), ScalarType::String)
                .with_attribute("source".to_string(), ScalarType::String),
        );

        let mut files = Relation::new(files_type);
        let mut deps = Relation::new(deps_type);

        // Files:
        // util.c (100)
        // util.h (110) <- recently modified!
        // util.o (105) <- out of date because of util.h
        // main.c (100)
        // main.o (105) <- up to date with main.c
        // app (missing) <- missing!

        files
            .insert(tuple! { name: "util.c", timestamp: 100i64 })
            .unwrap();
        files
            .insert(tuple! { name: "util.h", timestamp: 110i64 })
            .unwrap();
        files
            .insert(tuple! { name: "util.o", timestamp: 105i64 })
            .unwrap();
        files
            .insert(tuple! { name: "main.c", timestamp: 100i64 })
            .unwrap();
        files
            .insert(tuple! { name: "main.o", timestamp: 105i64 })
            .unwrap();

        // Dependencies:
        // app -> main.o
        // app -> util.o
        // main.o -> main.c
        // util.o -> util.c
        // util.o -> util.h
        // main.o -> util.h (let's say main includes util.h)

        deps.insert(tuple! { target: "app", source: "main.o" })
            .unwrap();
        deps.insert(tuple! { target: "app", source: "util.o" })
            .unwrap();
        deps.insert(tuple! { target: "main.o", source: "main.c" })
            .unwrap();
        deps.insert(tuple! { target: "util.o", source: "util.c" })
            .unwrap();
        deps.insert(tuple! { target: "util.o", source: "util.h" })
            .unwrap();
        deps.insert(tuple! { target: "main.o", source: "util.h" })
            .unwrap();

        let stale = get_stale_targets(&files, &deps).unwrap();

        // Stale should be:
        // app (missing)
        // util.o (older than util.h: 105 < 110)
        // main.o (older than util.h: 105 < 110)

        assert_eq!(stale.cardinality(), 3);

        let mut stale_names = vec![];
        for t in stale.tuples() {
            stale_names.push(t.get_typed::<String>("target").unwrap().to_string());
        }
        stale_names.sort();

        assert_eq!(stale_names, vec!["app", "main.o", "util.o"]);
    }
}
