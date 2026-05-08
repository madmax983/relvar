use relvar_core::{DatabaseError, Relation, RelationType, ScalarType, TupleType};

#[cfg(test)]
use relvar_core::tuple;

/// A Relational Package Manager.
///
/// This models a package manager's state (installed packages, dependencies,
/// explicitly installed roots) purely as relations, and computes unfulfilled
/// requirements, orphans, and conflicts using relational algebra.
#[derive(Debug, Clone)]
pub struct RelationalPackageManager {
    /// Schema: {package: String, version: String}
    pub installed: Relation,
    /// Schema: {package: String, dependency: String, required_version: String}
    pub dependencies: Relation,
    /// Schema: {package: String}
    pub explicitly_installed: Relation,
}

impl RelationalPackageManager {
    /// Creates a new, empty RelationalPackageManager.
    pub fn new() -> Self {
        let installed_type = RelationType::new(
            TupleType::new()
                .with_attribute("package", ScalarType::String)
                .with_attribute("version", ScalarType::String),
        );
        let deps_type = RelationType::new(
            TupleType::new()
                .with_attribute("package", ScalarType::String)
                .with_attribute("dependency", ScalarType::String)
                .with_attribute("required_version", ScalarType::String),
        );
        let explicit_type =
            RelationType::new(TupleType::new().with_attribute("package", ScalarType::String));

        Self {
            installed: Relation::new(installed_type),
            dependencies: Relation::new(deps_type),
            explicitly_installed: Relation::new(explicit_type),
        }
    }

    /// Finds packages that are required as dependencies but not currently installed.
    /// Returns a relation with schema: {package: String, dependency: String, required_version: String}
    pub fn find_unmet_dependencies(&self) -> Result<Relation, DatabaseError> {
        // Find dependencies for currently installed packages
        // {package, version} ⨝ {package, dependency, required_version}
        let active_deps = self.installed.join(&self.dependencies)?;

        // Project installed packages to match dependency names
        // {dependency} (by renaming package to dependency)
        let installed_names = self
            .installed
            .project(&["package"])
            .rename(&[("package", "dependency")]);

        // Unmet dependencies are those in active_deps where `dependency` is not in `installed_names`
        // Instead of a direct difference (schemas differ), we can semi-difference.
        // First, project out just the dependencies from active_deps
        let all_required_deps = active_deps.project(&["dependency"]);

        // Find which required deps are missing
        let missing_deps = all_required_deps
            .difference(&installed_names)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Now join back with active_deps to get the full information (package, required_version)
        let unmet = active_deps.join(&missing_deps)?;

        Ok(unmet)
    }

    /// Finds "orphan" packages that are installed but not explicitly installed
    /// and not required by any other installed package.
    /// Returns a relation with schema: {package: String}
    pub fn find_orphans(&self) -> Result<Relation, DatabaseError> {
        // 1. Get all installed package names
        let all_installed = self.installed.project(&["package"]);

        // 2. Get dependencies required by any installed package
        let active_deps = self.installed.join(&self.dependencies)?;
        let required_by_others = active_deps
            .project(&["dependency"])
            .rename(&[("dependency", "package")]);

        // 3. Combine explicitly installed and required_by_others (union)
        let needed_packages = self
            .explicitly_installed
            .union(&required_by_others)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Orphans are installed packages that are NOT in needed_packages
        let orphans = all_installed
            .difference(&needed_packages)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(orphans)
    }

    /// Finds packages with conflicting dependencies (e.g., A needs C v1.0, B needs C v2.0).
    /// Returns a relation with schema: {dependency: String}
    pub fn find_conflicts(&self) -> Result<Relation, DatabaseError> {
        let active_deps = self.installed.join(&self.dependencies)?;

        // We want to find `dependency` values that have multiple distinct `required_version`s.
        let deps_and_versions = active_deps.project(&["dependency", "required_version"]);

        // We can do this with a theta-join on itself: D1.dependency = D2.dependency AND D1.required_version != D2.required_version
        let d1 = deps_and_versions.rename(&[("required_version", "v1")]);
        let d2 = deps_and_versions.rename(&[("required_version", "v2")]);

        let cross = d1.join(&d2)?; // NATURAL JOIN on `dependency`

        let conflicts = cross.restrict(|t| {
            let v1 = t.get_typed::<String>("v1").unwrap();
            let v2 = t.get_typed::<String>("v2").unwrap();
            v1 != v2
        });

        // Project out just the dependency name
        let conflicting_deps = conflicts.project(&["dependency"]);

        Ok(conflicting_deps)
    }
}

impl Default for RelationalPackageManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_unmet_dependencies() {
        let mut pm = RelationalPackageManager::new();

        // Installed: A, B
        pm.installed
            .insert(tuple! { package: "A", version: "1.0" })
            .unwrap();
        pm.installed
            .insert(tuple! { package: "B", version: "2.0" })
            .unwrap();

        // Dependencies: A depends on C, B depends on D
        pm.dependencies
            .insert(tuple! { package: "A", dependency: "C", required_version: "1.0" })
            .unwrap();
        pm.dependencies
            .insert(tuple! { package: "B", dependency: "D", required_version: "2.0" })
            .unwrap();
        // C is not installed. D is not installed.

        let unmet = pm.find_unmet_dependencies().unwrap();
        assert_eq!(unmet.cardinality(), 2);

        let mut deps = Vec::new();
        for t in unmet.tuples() {
            deps.push(t.get_typed::<String>("dependency").unwrap().clone());
        }
        deps.sort();
        assert_eq!(deps, vec!["C", "D"]);
    }

    #[test]
    fn test_find_orphans() {
        let mut pm = RelationalPackageManager::new();

        pm.installed
            .insert(tuple! { package: "App", version: "1.0" })
            .unwrap();
        pm.installed
            .insert(tuple! { package: "Lib", version: "1.0" })
            .unwrap();
        pm.installed
            .insert(tuple! { package: "Orphan", version: "1.0" })
            .unwrap();

        pm.explicitly_installed
            .insert(tuple! { package: "App" })
            .unwrap();
        pm.dependencies
            .insert(tuple! { package: "App", dependency: "Lib", required_version: "1.0" })
            .unwrap();

        let orphans = pm.find_orphans().unwrap();
        assert_eq!(orphans.cardinality(), 1);
        let orphan_tuple = orphans.tuples().next().unwrap();
        assert_eq!(
            orphan_tuple.get_typed::<String>("package").unwrap(),
            "Orphan"
        );
    }

    #[test]
    fn test_find_conflicts() {
        let mut pm = RelationalPackageManager::new();

        pm.installed
            .insert(tuple! { package: "App1", version: "1.0" })
            .unwrap();
        pm.installed
            .insert(tuple! { package: "App2", version: "1.0" })
            .unwrap();

        // Conflict on "Lib"
        pm.dependencies
            .insert(tuple! { package: "App1", dependency: "Lib", required_version: "1.0" })
            .unwrap();
        pm.dependencies
            .insert(tuple! { package: "App2", dependency: "Lib", required_version: "2.0" })
            .unwrap();

        let conflicts = pm.find_conflicts().unwrap();
        assert_eq!(conflicts.cardinality(), 1);
        let conflict_tuple = conflicts.tuples().next().unwrap();
        assert_eq!(
            conflict_tuple.get_typed::<String>("dependency").unwrap(),
            "Lib"
        );
    }
}
