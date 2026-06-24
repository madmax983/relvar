//! Relational Build System Simulator.
//!
//! This module implements a build system (like Make) using pure relational algebra.
//! Dependencies and file timestamps are represented as relations, and stale targets
//! are computed declaratively using transitive closure, joins, and set differences.

use crate::error::DatabaseError;
use crate::values::Relation;

/// Computes the set of stale targets that need to be rebuilt.
///
/// `dependencies` must have attributes: `target` (String), `source` (String).
/// `timestamps` must have attributes: `file` (String), `ts` (Int).
///
/// Returns a relation with a single attribute `target` (String) representing
/// the files that need to be rebuilt.
///
/// # Examples
/// ```
/// use relvar_core::tuple;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar_core::experimental::build_system::compute_stale_targets;
///
/// // Create dependencies relation
/// let dep_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("target", ScalarType::String)
///         .with_attribute("source", ScalarType::String)
/// );
/// let mut deps = Relation::new(dep_type);
/// deps.insert(tuple! { target: "main.o".to_string(), source: "main.c".to_string() }).unwrap();
/// deps.insert(tuple! { target: "main.o".to_string(), source: "main.h".to_string() }).unwrap();
/// deps.insert(tuple! { target: "app".to_string(), source: "main.o".to_string() }).unwrap();
///
/// // Create timestamps relation
/// let ts_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("file", ScalarType::String)
///         .with_attribute("ts", ScalarType::Int)
/// );
/// let mut ts = Relation::new(ts_type);
/// // main.c is newer (100) than main.o (50)
/// ts.insert(tuple! { file: "main.c".to_string(), ts: 100i64 }).unwrap();
/// ts.insert(tuple! { file: "main.h".to_string(), ts: 10i64 }).unwrap();
/// ts.insert(tuple! { file: "main.o".to_string(), ts: 50i64 }).unwrap();
/// ts.insert(tuple! { file: "app".to_string(), ts: 60i64 }).unwrap();
///
/// // app is newer than main.o, but because main.c > main.o, main.o is stale.
/// // Because main.o is stale, it will get a new timestamp > 100.
/// // But wait! Our query checks TRANSITIVE dependencies!
/// // app depends on main.c transitively, and main.c (100) > app (60).
/// // So app is ALSO stale!
///
/// let stale = compute_stale_targets(&deps, &ts).unwrap();
/// assert_eq!(stale.cardinality(), 2);
/// ```
pub fn compute_stale_targets(
    dependencies: &Relation,
    timestamps: &Relation,
) -> Result<Relation, DatabaseError> {
    // 1. Compute transitive dependencies
    let transitive_deps = dependencies.tclose("target", "source")?;

    // 2. Find targets that don't have a timestamp yet (missing files)
    let all_targets = dependencies.project(&["target"]);
    let known_files = timestamps
        .project(&["file"])
        .rename_into(&[("file", "target")]);
    let missing_targets = all_targets
        .difference(&known_files)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 3. Find targets whose sources are newer
    let source_ts = timestamps.rename(&[("file", "source"), ("ts", "source_ts")]);
    let target_ts = timestamps.rename(&[("file", "target"), ("ts", "target_ts")]);

    let deps_with_source_ts = transitive_deps
        .join(&source_ts)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
    let deps_with_both_ts = deps_with_source_ts
        .join(&target_ts)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    let out_of_date = deps_with_both_ts.restrict(|t| {
        let s_ts = t.get_typed::<i64>("source_ts").unwrap_or(0);
        let t_ts = t.get_typed::<i64>("target_ts").unwrap_or(0);
        s_ts > t_ts
    });

    let stale_existing = out_of_date.project(&["target"]);

    // 4. Combine missing targets and out of date targets
    let all_stale = missing_targets
        .union(&stale_existing)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    Ok(all_stale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_compute_stale_targets() {
        let dep_type = RelationType::new(
            TupleType::new()
                .with_attribute("target", ScalarType::String)
                .with_attribute("source", ScalarType::String),
        );
        let mut deps = Relation::new(dep_type);
        deps.insert(tuple! { target: "main.o".to_string(), source: "main.c".to_string() })
            .unwrap();
        deps.insert(tuple! { target: "main.o".to_string(), source: "main.h".to_string() })
            .unwrap();
        deps.insert(tuple! { target: "app".to_string(), source: "main.o".to_string() })
            .unwrap();

        let ts_type = RelationType::new(
            TupleType::new()
                .with_attribute("file", ScalarType::String)
                .with_attribute("ts", ScalarType::Int),
        );
        let mut ts = Relation::new(ts_type);
        ts.insert(tuple! { file: "main.c".to_string(), ts: 100i64 })
            .unwrap();
        ts.insert(tuple! { file: "main.h".to_string(), ts: 10i64 })
            .unwrap();
        ts.insert(tuple! { file: "main.o".to_string(), ts: 50i64 })
            .unwrap();
        ts.insert(tuple! { file: "app".to_string(), ts: 60i64 })
            .unwrap();

        let stale = compute_stale_targets(&deps, &ts).unwrap();
        assert_eq!(stale.cardinality(), 2);

        let mut stale_targets: Vec<String> = stale
            .tuples()
            .map(|t| t.get_typed::<String>("target").unwrap())
            .collect();
        stale_targets.sort();

        assert_eq!(stale_targets, vec!["app".to_string(), "main.o".to_string()]);
    }
}
