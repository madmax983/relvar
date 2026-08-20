//! A relational approach to a Build System (like Make/Ninja).
//!
//! This module demonstrates how to resolve stale targets in a build graph
//! using purely relational algebra, specifically leveraging transitive closure
//! (`tclose`) to find all indirect dependencies, and relational joins to
//! compare file modification times.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Identifies all stale targets that need to be rebuilt.
///
/// A target is considered stale if:
/// 1. It is declared as a target in the dependencies graph but is missing from timestamps.
/// 2. It has an older modification time than any of its direct or indirect dependencies.
///
/// # Arguments
///
/// * `dependencies` - A binary relation with attributes `target` (String) and `source` (String).
/// * `timestamps` - A binary relation with attributes `file` (String) and `mtime` (Int).
///
/// # Returns
///
/// A relation with a single attribute `target` (String) containing all stale targets.
pub fn find_stale_targets(
    dependencies: &Relation,
    timestamps: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Compute all transitive dependencies (direct and indirect)
    let all_deps = dependencies.tclose("target", "source")?;

    // 2. Find targets missing from timestamps
    let all_targets = dependencies.project(&["target"]);
    let timestamped_files = timestamps.project(&["file"]).rename(&[("file", "target")]);
    let missing_targets = all_targets
        .difference_into(&timestamped_files)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 3. Compare modification times for existing targets
    // Join all_deps with timestamps to get target_mtime
    let target_times = all_deps
        .rename(&[("target", "file")])
        .join(timestamps)?
        .rename(&[("file", "target"), ("mtime", "target_mtime")]);

    // Join with timestamps again to get source_mtime
    let both_times = target_times
        .rename(&[("source", "file")])
        .join(timestamps)?
        .rename(&[("file", "source"), ("mtime", "source_mtime")]);

    // Filter to those where target is older than source
    let out_of_date = both_times
        .restrict(|t| {
            let target_mtime = t.get_typed::<i64>("target_mtime").unwrap_or(0);
            let source_mtime = t.get_typed::<i64>("source_mtime").unwrap_or(0);
            target_mtime < source_mtime
        })
        .project(&["target"]);

    // 4. Union missing targets and out-of-date targets
    out_of_date
        .union_into(&missing_targets)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_build_system() {
        let dep_type = RelationType::new(
            TupleType::new()
                .with_attribute("target", ScalarType::String)
                .with_attribute("source", ScalarType::String),
        );
        let mut deps = Relation::new(dep_type);
        deps.insert(tuple! { target: "main.o", source: "main.c" })
            .unwrap();
        deps.insert(tuple! { target: "main.o", source: "main.h" })
            .unwrap();
        deps.insert(tuple! { target: "app", source: "main.o" })
            .unwrap();

        let time_type = RelationType::new(
            TupleType::new()
                .with_attribute("file", ScalarType::String)
                .with_attribute("mtime", ScalarType::Int),
        );
        let mut times = Relation::new(time_type);
        // main.c is 100
        times
            .insert(tuple! { file: "main.c", mtime: 100i64 })
            .unwrap();
        // main.h is 150 (recently modified!)
        times
            .insert(tuple! { file: "main.h", mtime: 150i64 })
            .unwrap();
        // main.o was built at 120 (older than main.h)
        times
            .insert(tuple! { file: "main.o", mtime: 120i64 })
            .unwrap();
        // app was built at 130 (newer than main.o, but older than main.h transitively!)
        times.insert(tuple! { file: "app", mtime: 130i64 }).unwrap();

        let stale = find_stale_targets(&deps, &times).unwrap();

        // Both main.o and app should be stale
        assert_eq!(stale.cardinality(), 2);

        let mut targets: Vec<String> = stale
            .tuples()
            .map(|t| t.get_typed::<String>("target").unwrap())
            .collect();
        targets.sort();
        assert_eq!(targets, vec!["app".to_string(), "main.o".to_string()]);
    }

    #[test]
    fn test_missing_target() {
        let dep_type = RelationType::new(
            TupleType::new()
                .with_attribute("target", ScalarType::String)
                .with_attribute("source", ScalarType::String),
        );
        let mut deps = Relation::new(dep_type);
        deps.insert(tuple! { target: "main.o", source: "main.c" })
            .unwrap();

        let time_type = RelationType::new(
            TupleType::new()
                .with_attribute("file", ScalarType::String)
                .with_attribute("mtime", ScalarType::Int),
        );
        let mut times = Relation::new(time_type);
        times
            .insert(tuple! { file: "main.c", mtime: 100i64 })
            .unwrap();
        // main.o is missing from times

        let stale = find_stale_targets(&deps, &times).unwrap();
        assert_eq!(stale.cardinality(), 1);
        assert!(stale.contains(&tuple! { target: "main.o" }));
    }
}
