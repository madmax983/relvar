//! Relational Build System.
//!
//! Models a build system (like Make/Ninja) using pure relational algebra.
//! Stale targets are resolved by finding files older than their dependencies via
//! transitive closure (`tclose`), relational joins, and restrictions.

use crate::error::DatabaseError;
use crate::values::{Relation, Tuple};

/// Finds stale targets that need to be rebuilt.
///
/// * `deps`: A binary relation with attributes `target` (String) and `dependency` (String).
/// * `times`: A binary relation with attributes `file` (String) and `timestamp` (Int).
///
/// Returns a unary relation with attribute `target` (String) containing the stale targets.
pub fn find_stale_targets(deps: &Relation, times: &Relation) -> Result<Relation, DatabaseError> {
    // 1. Find all transitive dependencies using transitive closure
    let all_deps = deps.tclose("target", "dependency")?;

    // 2. Rename times to match target attribute
    let times_target = times.rename(&[("file", "target"), ("timestamp", "target_time")]);

    // 3. Rename times to match dependency attribute
    let times_dep = times.rename(&[("file", "dependency"), ("timestamp", "dep_time")]);

    // 4. Join transitive dependencies with timestamps
    let joined = all_deps.join(&times_target)?.join(&times_dep)?;

    // 5. Filter for targets that are older than their dependencies
    let stale = joined.restrict(|t: &Tuple| {
        let target_time = t.get_typed::<i64>("target_time").unwrap_or(0);
        let dep_time = t.get_typed::<i64>("dep_time").unwrap_or(0);
        target_time < dep_time
    });

    // 6. Project to just the target names
    let result = stale.project(&["target"]);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_find_stale_targets() {
        // Setup deps relation
        let deps_heading = TupleType::new()
            .with_attribute("target", ScalarType::String)
            .with_attribute("dependency", ScalarType::String);
        let mut deps = Relation::new(RelationType::new(deps_heading));

        // main.o depends on main.c and header.h
        deps.insert(tuple! { target: "main.o", dependency: "main.c" })
            .unwrap();
        deps.insert(tuple! { target: "main.o", dependency: "header.h" })
            .unwrap();

        // program depends on main.o
        deps.insert(tuple! { target: "program", dependency: "main.o" })
            .unwrap();

        // Setup times relation
        let times_heading = TupleType::new()
            .with_attribute("file", ScalarType::String)
            .with_attribute("timestamp", ScalarType::Int);
        let mut times = Relation::new(RelationType::new(times_heading));

        times
            .insert(tuple! { file: "header.h", timestamp: 200i64 })
            .unwrap();
        times
            .insert(tuple! { file: "main.c", timestamp: 100i64 })
            .unwrap();
        times
            .insert(tuple! { file: "main.o", timestamp: 150i64 })
            .unwrap(); // Older than header.h!
        times
            .insert(tuple! { file: "program", timestamp: 180i64 })
            .unwrap(); // Newer than main.o, but older than header.h!

        let stale_targets = find_stale_targets(&deps, &times).unwrap();

        // Both main.o and program should be stale because header.h is 200
        assert_eq!(stale_targets.cardinality(), 2);
        assert!(stale_targets.contains(&tuple! { target: "main.o" }));
        assert!(stale_targets.contains(&tuple! { target: "program" }));
    }
}
