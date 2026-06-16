//! Relational Build System.

use crate::database::Database;
use crate::error::DatabaseError;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;

/// A Build System evaluated purely using Relational Algebra.
pub struct BuildSystem {
    db: Database<InMemoryEngine>,
}

impl Default for BuildSystem {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

impl BuildSystem {
    /// Creates a new Build System.
    pub fn new() -> Result<Self, DatabaseError> {
        let mut db = Database::new(InMemoryEngine::new());

        let rules_type = RelationType::new(
            TupleType::new()
                .with_attribute("target", ScalarType::String)
                .with_attribute("dependency", ScalarType::String),
        );
        db.create_relvar("rules", rules_type)?;

        let files_type = RelationType::new(
            TupleType::new()
                .with_attribute("path", ScalarType::String)
                .with_attribute("mtime", ScalarType::Int),
        );
        db.create_relvar("files", files_type)?;

        Ok(Self { db })
    }

    /// Adds a dependency rule.
    pub fn add_rule(&mut self, target: &str, dependency: &str) -> Result<(), DatabaseError> {
        self.db.insert(
            "rules",
            tuple! {
                target: target.to_string(),
                dependency: dependency.to_string()
            },
        )?;
        Ok(())
    }

    /// Updates a file's modification time.
    pub fn update_file(&mut self, path: &str, mtime: i64) -> Result<(), DatabaseError> {
        self.db.insert(
            "files",
            tuple! {
                path: path.to_string(),
                mtime: mtime
            },
        )?;
        Ok(())
    }

    /// Evaluates stale targets using relational algebra.
    pub fn evaluate_stale_targets(&self) -> Result<Relation, DatabaseError> {
        let rules = self.db.query("rules")?;
        let files = self.db.query("files")?;

        let transitive_rules = rules.tclose("target", "dependency")?;

        let targets_with_mtime = transitive_rules
            .join(&files.rename(&[("path", "target"), ("mtime", "target_mtime")]))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let all_with_mtime = targets_with_mtime
            .join(&files.rename(&[("path", "dependency"), ("mtime", "dep_mtime")]))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let out_of_date = all_with_mtime
            .restrict(|t| {
                let t_mtime = t.get_typed::<i64>("target_mtime").unwrap_or(0);
                let d_mtime = t.get_typed::<i64>("dep_mtime").unwrap_or(0);
                d_mtime > t_mtime
            })
            .project(&["target"]);

        let missing_targets = rules
            .project(&["target"])
            .difference(&files.rename(&[("path", "target")]).project(&["target"]))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        out_of_date
            .union(&missing_targets)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_system() {
        let mut build = BuildSystem::new().unwrap();

        build.add_rule("app", "main.o").unwrap();
        build.add_rule("app", "lib.o").unwrap();
        build.add_rule("main.o", "main.c").unwrap();
        build.add_rule("main.o", "common.h").unwrap();
        build.add_rule("lib.o", "lib.c").unwrap();
        build.add_rule("lib.o", "common.h").unwrap();

        build.update_file("app", 100).unwrap();
        build.update_file("main.o", 90).unwrap();
        build.update_file("lib.o", 95).unwrap();
        build.update_file("main.c", 80).unwrap();
        build.update_file("lib.c", 85).unwrap();
        build.update_file("common.h", 70).unwrap();

        let stale = build.evaluate_stale_targets().unwrap();
        assert_eq!(stale.cardinality(), 0);

        build.update_file("common.h", 110).unwrap();

        let stale = build.evaluate_stale_targets().unwrap();
        assert_eq!(stale.cardinality(), 3); // main.o, lib.o, app

        let mut has_app = false;
        let mut has_main = false;
        let mut has_lib = false;
        for t in stale.tuples() {
            let target = t.get_typed::<String>("target").unwrap();
            if target == "app" {
                has_app = true;
            }
            if target == "main.o" {
                has_main = true;
            }
            if target == "lib.o" {
                has_lib = true;
            }
        }
        assert!(has_app);
        assert!(has_main);
        assert!(has_lib);
    }

    #[test]
    fn test_missing_target() {
        let mut build = BuildSystem::new().unwrap();
        build.add_rule("app", "main.o").unwrap();
        build.update_file("main.o", 100).unwrap();

        let stale = build.evaluate_stale_targets().unwrap();
        assert_eq!(stale.cardinality(), 1);
        let t = stale.tuples().next().unwrap();
        assert_eq!(t.get_typed::<String>("target").unwrap(), "app");
    }
}
