//! Relational Build System Simulator.
//!
//! This module models a build system (like Make or Ninja) using pure relational algebra.
//! Dependencies are modeled as an edges relation, and file timestamps are a relation.
//! Stale targets are resolved by finding files older than their dependencies via
//! transitive closure (`tclose`), relational joins, and aggregations.

use crate::algebra::Aggregation;
use crate::error::DatabaseError;
use crate::types::ScalarType;
use crate::values::Relation;

/// Finds stale build targets that need recompilation based on modified dependencies.
///
/// `dependencies` must have attributes: `target` (String), `dependency` (String).
/// `timestamps` must have attributes: `file` (String), `timestamp` (Int).
///
/// # Examples
/// ```
/// use relvar_core::tuple;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar_core::experimental::build_system::find_stale_targets;
///
/// let dep_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("target", ScalarType::String)
///         .with_attribute("dependency", ScalarType::String)
/// );
/// let mut deps = Relation::new(dep_type);
/// // main.o depends on main.c and common.h
/// deps.insert(tuple! { target: "main.o", dependency: "main.c" }).unwrap();
/// deps.insert(tuple! { target: "main.o", dependency: "common.h" }).unwrap();
/// // app depends on main.o
/// deps.insert(tuple! { target: "app", dependency: "main.o" }).unwrap();
///
/// let time_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("file", ScalarType::String)
///         .with_attribute("timestamp", ScalarType::Int)
/// );
/// let mut times = Relation::new(time_type);
/// times.insert(tuple! { file: "main.c", timestamp: 100i64 }).unwrap();
/// times.insert(tuple! { file: "common.h", timestamp: 200i64 }).unwrap();
/// times.insert(tuple! { file: "main.o", timestamp: 150i64 }).unwrap(); // Stale: older than common.h
/// times.insert(tuple! { file: "app", timestamp: 160i64 }).unwrap(); // Stale: also older than common.h
///
/// let stale = find_stale_targets(&deps, &times).unwrap();
/// assert_eq!(stale.cardinality(), 2);
/// assert!(stale.contains(&tuple! { target: "main.o" }));
/// assert!(stale.contains(&tuple! { target: "app" }));
/// ```
pub fn find_stale_targets(
    dependencies: &Relation,
    timestamps: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Find all transitive dependencies: (target, dependency)
    let all_deps = dependencies.tclose("target", "dependency")?;

    // 2. Union direct and transitive dependencies
    let full_deps = dependencies.union(&all_deps)
       .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 3. Join with timestamps to get dependency timestamps
    // We rename 'file' to 'dependency' in timestamps
    let dep_times = timestamps.rename(&[("file", "dependency")]);

    // Join full_deps with dep_times => (target, dependency, timestamp)
    let deps_with_times = full_deps.join(&dep_times)?;

    // 4. Summarize to find the maximum timestamp among all dependencies for a target
    let max_dep_times = deps_with_times
        .summarize(
            &["target"],
            &[Aggregation::max("max_dep_timestamp", "timestamp", ScalarType::Int)]
        )
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 5. Join the target's own timestamp
    let target_times = timestamps.rename(&[("file", "target")]);

    // => (target, max_dep_timestamp, timestamp)  (timestamp here is the target's own time)
    let targets_with_both_times = max_dep_times.join(&target_times)?;

    // 6. Restrict to targets where max_dep_timestamp > target's timestamp
    let stale = targets_with_both_times.restrict(|t| {
        let max_dep = t.get_typed::<i64>("max_dep_timestamp").unwrap_or(0);
        let my_time = t.get_typed::<i64>("timestamp").unwrap_or(0);
        max_dep > my_time
    });

    // 7. Project out just the target name
    Ok(stale.project(&["target"]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, TupleType};

    #[test]
    fn test_stale_targets_direct() {
        let dep_heading = TupleType::new()
            .with_attribute("target", ScalarType::String)
            .with_attribute("dependency", ScalarType::String);
        let mut deps = Relation::new(RelationType::new(dep_heading));
        deps.insert(tuple! { target: "a.o", dependency: "a.c" }).unwrap();

        let time_heading = TupleType::new()
            .with_attribute("file", ScalarType::String)
            .with_attribute("timestamp", ScalarType::Int);
        let mut times = Relation::new(RelationType::new(time_heading));
        times.insert(tuple! { file: "a.c", timestamp: 100i64 }).unwrap();
        times.insert(tuple! { file: "a.o", timestamp: 50i64 }).unwrap();

        let stale = find_stale_targets(&deps, &times).unwrap();
        assert_eq!(stale.cardinality(), 1);
        assert!(stale.contains(&tuple! { target: "a.o" }));
    }

    #[test]
    fn test_stale_targets_transitive() {
        let dep_heading = TupleType::new()
            .with_attribute("target", ScalarType::String)
            .with_attribute("dependency", ScalarType::String);
        let mut deps = Relation::new(RelationType::new(dep_heading));
        deps.insert(tuple! { target: "app", dependency: "a.o" }).unwrap();
        deps.insert(tuple! { target: "a.o", dependency: "a.c" }).unwrap();

        let time_heading = TupleType::new()
            .with_attribute("file", ScalarType::String)
            .with_attribute("timestamp", ScalarType::Int);
        let mut times = Relation::new(RelationType::new(time_heading));

        // a.c was just modified (time=200)
        times.insert(tuple! { file: "a.c", timestamp: 200i64 }).unwrap();
        // a.o is up to date with its old a.c (time=100)
        times.insert(tuple! { file: "a.o", timestamp: 100i64 }).unwrap();
        // app is up to date with a.o (time=150)
        times.insert(tuple! { file: "app", timestamp: 150i64 }).unwrap();

        // BOTH a.o and app should be stale, because app transitively depends on a.c (time 200) which is > app's time (150)
        let stale = find_stale_targets(&deps, &times).unwrap();
        assert_eq!(stale.cardinality(), 2);
        assert!(stale.contains(&tuple! { target: "a.o" }));
        assert!(stale.contains(&tuple! { target: "app" }));
    }

    #[test]
    fn test_no_stale_targets() {
        let dep_heading = TupleType::new()
            .with_attribute("target", ScalarType::String)
            .with_attribute("dependency", ScalarType::String);
        let mut deps = Relation::new(RelationType::new(dep_heading));
        deps.insert(tuple! { target: "a.o", dependency: "a.c" }).unwrap();

        let time_heading = TupleType::new()
            .with_attribute("file", ScalarType::String)
            .with_attribute("timestamp", ScalarType::Int);
        let mut times = Relation::new(RelationType::new(time_heading));
        times.insert(tuple! { file: "a.c", timestamp: 50i64 }).unwrap();
        times.insert(tuple! { file: "a.o", timestamp: 100i64 }).unwrap();

        let stale = find_stale_targets(&deps, &times).unwrap();
        assert_eq!(stale.cardinality(), 0);
    }
}
