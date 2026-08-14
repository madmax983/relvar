//! Relational Version Control System (RelGit)
//!
//! This module demonstrates how a version control system similar to Git can be implemented using purely
//! relational algebra operations. It represents a commit history, file blobs, directory trees, and branches
//! as relations. Operations like `checkout` and `diff` are computed declaratively using Relvar's algebra.
//!
//! # Concept
//!
//! We define four fundamental relations:
//! - **Blobs**: `(hash: String, content: String)`
//! - **Trees**: `(tree_hash: String, path: String, blob_hash: String)`
//! - **Commits**: `(commit_hash: String, parent_hash: String, tree_hash: String, message: String)`
//! - **Branches**: `(name: String, commit_hash: String)`
//!
//! Using relational algebra, we can:
//! - **Checkout**: Join `Branches ⨝ Commits ⨝ Trees ⨝ Blobs` to reconstruct the working directory.
//! - **Diff**: Join two trees, extend to find differences, and restrict to see added, removed, or changed files.

use relvar_core::{
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue},
};

/// A relational Version Control System (VCS).
/// # Examples
///
/// ```
/// use relvar::{Database, InMemoryEngine, Relation, RelationType, ScalarType, TupleType};
/// // Note: This is a placeholder example
/// ```
pub struct RelVcs {
    /// Blobs: (hash: String, content: String)
    pub blobs: Relation,
    /// Trees: (tree_hash: String, path: String, blob_hash: String)
    pub trees: Relation,
    /// Commits: (commit_hash: String, parent_hash: String, tree_hash: String, message: String)
    pub commits: Relation,
    /// Branches: (name: String, commit_hash: String)
    pub branches: Relation,
}

impl RelVcs {
    /// Creates a new, empty RelVcs.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Database, InMemoryEngine, Relation, RelationType, ScalarType, TupleType};
    /// // Note: This is a placeholder example
    /// ```
    pub fn new() -> Self {
        let blobs_type = RelationType::new(
            TupleType::new()
                .with_attribute("hash".to_string(), ScalarType::String)
                .with_attribute("content".to_string(), ScalarType::String),
        );

        let trees_type = RelationType::new(
            TupleType::new()
                .with_attribute("tree_hash".to_string(), ScalarType::String)
                .with_attribute("path".to_string(), ScalarType::String)
                .with_attribute("blob_hash".to_string(), ScalarType::String),
        );

        let commits_type = RelationType::new(
            TupleType::new()
                .with_attribute("commit_hash".to_string(), ScalarType::String)
                .with_attribute("parent_hash".to_string(), ScalarType::String)
                .with_attribute("tree_hash".to_string(), ScalarType::String)
                .with_attribute("message".to_string(), ScalarType::String),
        );

        let branches_type = RelationType::new(
            TupleType::new()
                .with_attribute("name".to_string(), ScalarType::String)
                .with_attribute("commit_hash".to_string(), ScalarType::String),
        );

        Self {
            blobs: Relation::new(blobs_type),
            trees: Relation::new(trees_type),
            commits: Relation::new(commits_type),
            branches: Relation::new(branches_type),
        }
    }

    /// Checks out the specified branch.
    /// Computes and returns a relation representing the working directory: (path: String, content: String)
    /// # Examples
    ///
    /// ```
    /// use relvar::{Database, InMemoryEngine, Relation, RelationType, ScalarType, TupleType};
    /// // Note: This is a placeholder example
    /// ```
    pub fn checkout(&self, branch_name: &str) -> Result<Relation, DatabaseError> {
        // 1. Restrict branches to the given branch_name
        let branch = self
            .branches
            .restrict(|t| t.get_typed::<String>("name").unwrap() == branch_name);

        if branch.cardinality() == 0 {
            return Err(DatabaseError::AlgebraError(format!(
                "Branch '{}' not found",
                branch_name
            )));
        }

        // 2. Join Branches ⨝ Commits to get the target commit
        // Commits: (commit_hash, parent_hash, tree_hash, message)
        // Branch: (name, commit_hash)
        let commit = branch.join(&self.commits)?;

        // 3. Join Commit ⨝ Trees to get the files in the commit
        // Tree: (tree_hash, path, blob_hash)
        let tree = commit
            .join(&self.trees)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Join Tree ⨝ Blobs to get the file contents
        // Blob: (hash, content)
        // Wait, Trees has 'blob_hash' and Blobs has 'hash'. We need to rename.
        let blobs_renamed = self.blobs.rename(&[("hash", "blob_hash")]);
        let working_dir_full = tree
            .join(&blobs_renamed)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Project to just (path, content)
        let working_dir = working_dir_full.project(&["path", "content"]);

        Ok(working_dir)
    }

    /// Computes the diff between two commits.
    /// Computes and returns a relation containing changes: (path: String, status: String)
    /// Statuses: "added", "removed", "modified"
    /// # Examples
    ///
    /// ```
    /// use relvar::{Database, InMemoryEngine, Relation, RelationType, ScalarType, TupleType};
    /// // Note: This is a placeholder example
    /// ```
    pub fn diff(
        &self,
        commit_a_hash: &str,
        commit_b_hash: &str,
    ) -> Result<Relation, DatabaseError> {
        let tree_a = self.get_tree_for_commit(commit_a_hash)?;
        let tree_b = self.get_tree_for_commit(commit_b_hash)?;

        let paths_a = tree_a.project(&["path"]);
        let paths_b = tree_b.project(&["path"]);

        let added = self.compute_added_files(&paths_a, &paths_b)?;
        let removed = self.compute_removed_files(&paths_a, &paths_b)?;
        let modified = self.compute_modified_files(&tree_a, &tree_b)?;

        // 6. Union all differences
        let diff1 = added
            .union(&removed)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let full_diff = diff1
            .union(&modified)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(full_diff)
    }

    fn get_tree_for_commit(&self, commit_hash: &str) -> Result<Relation, DatabaseError> {
        let commit = self
            .commits
            .restrict(|t| t.get_typed::<String>("commit_hash").unwrap() == commit_hash);
        if commit.cardinality() == 0 {
            return Err(DatabaseError::AlgebraError(format!(
                "Commit '{}' not found",
                commit_hash
            )));
        }
        Ok(commit
            .join(&self.trees)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["path", "blob_hash"]))
    }

    fn compute_added_files(
        &self,
        paths_a: &Relation,
        paths_b: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // 3. Find Added Files (In B but not in A by path)
        let added_paths = paths_b
            .difference(paths_a)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        added_paths
            .extend("status", ScalarType::String, |_| {
                ScalarValue::String("added".to_string())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn compute_removed_files(
        &self,
        paths_a: &Relation,
        paths_b: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // 4. Find Removed Files (In A but not in B by path)
        let removed_paths = paths_a
            .difference(paths_b)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        removed_paths
            .extend("status", ScalarType::String, |_| {
                ScalarValue::String("removed".to_string())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn compute_modified_files(
        &self,
        tree_a: &Relation,
        tree_b: &Relation,
    ) -> Result<Relation, DatabaseError> {
        // 5. Find Modified Files (In both, but blob_hash differs)
        let tree_a_renamed = tree_a.rename(&[("blob_hash", "blob_hash_a")]);
        let tree_b_renamed = tree_b.rename(&[("blob_hash", "blob_hash_b")]);

        let both_trees = tree_a_renamed
            .join(&tree_b_renamed)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let modified_files = both_trees.restrict(|t| {
            t.get_typed::<String>("blob_hash_a").unwrap()
                != t.get_typed::<String>("blob_hash_b").unwrap()
        });

        let modified_paths = modified_files.project(&["path"]);
        modified_paths
            .extend("status", ScalarType::String, |_| {
                ScalarValue::String("modified".to_string())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }
}

impl Default for RelVcs {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_vcs_checkout_and_diff() {
        let mut vcs = RelVcs::new();

        // Blobs
        vcs.blobs
            .insert(tuple! { hash: "b1", content: "hello world" })
            .unwrap();
        vcs.blobs
            .insert(tuple! { hash: "b2", content: "hello universe" })
            .unwrap();
        vcs.blobs
            .insert(tuple! { hash: "b3", content: "new file" })
            .unwrap();

        // Commit 1 Tree (Initial commit: a.txt)
        vcs.trees
            .insert(tuple! { tree_hash: "t1", path: "a.txt", blob_hash: "b1" })
            .unwrap();
        vcs.commits.insert(tuple! { commit_hash: "c1", parent_hash: "", tree_hash: "t1", message: "initial commit" }).unwrap();

        // Commit 2 Tree (Modify a.txt, Add b.txt)
        vcs.trees
            .insert(tuple! { tree_hash: "t2", path: "a.txt", blob_hash: "b2" })
            .unwrap();
        vcs.trees
            .insert(tuple! { tree_hash: "t2", path: "b.txt", blob_hash: "b3" })
            .unwrap();
        vcs.commits
            .insert(
                tuple! { commit_hash: "c2", parent_hash: "c1", tree_hash: "t2", message: "update" },
            )
            .unwrap();

        // Branches
        vcs.branches
            .insert(tuple! { name: "main", commit_hash: "c2" })
            .unwrap();
        vcs.branches
            .insert(tuple! { name: "v1.0", commit_hash: "c1" })
            .unwrap();

        // Test checkout main
        let working_dir_main = vcs.checkout("main").unwrap();
        assert_eq!(working_dir_main.cardinality(), 2);

        let mut main_files = std::collections::HashMap::new();
        for t in working_dir_main.tuples() {
            main_files.insert(
                t.get_typed::<String>("path").unwrap().clone(),
                t.get_typed::<String>("content").unwrap().clone(),
            );
        }
        assert_eq!(main_files.get("a.txt").unwrap(), "hello universe");
        assert_eq!(main_files.get("b.txt").unwrap(), "new file");

        // Test checkout v1.0
        let working_dir_v1 = vcs.checkout("v1.0").unwrap();
        assert_eq!(working_dir_v1.cardinality(), 1);
        let first_file = working_dir_v1.tuples().next().unwrap();
        assert_eq!(first_file.get_typed::<String>("path").unwrap(), "a.txt");
        assert_eq!(
            first_file.get_typed::<String>("content").unwrap(),
            "hello world"
        );

        // Test Diff from c1 to c2
        let diff = vcs.diff("c1", "c2").unwrap();
        assert_eq!(diff.cardinality(), 2);

        let mut diff_results = std::collections::HashMap::new();
        for t in diff.tuples() {
            diff_results.insert(
                t.get_typed::<String>("path").unwrap().clone(),
                t.get_typed::<String>("status").unwrap().clone(),
            );
        }

        assert_eq!(diff_results.get("a.txt").unwrap(), "modified");
        assert_eq!(diff_results.get("b.txt").unwrap(), "added");

        // Test Diff from c2 to c1 (Reverse)
        let diff_rev = vcs.diff("c2", "c1").unwrap();
        assert_eq!(diff_rev.cardinality(), 2);
        let mut diff_rev_results = std::collections::HashMap::new();
        for t in diff_rev.tuples() {
            diff_rev_results.insert(
                t.get_typed::<String>("path").unwrap().clone(),
                t.get_typed::<String>("status").unwrap().clone(),
            );
        }

        assert_eq!(diff_rev_results.get("a.txt").unwrap(), "modified");
        assert_eq!(diff_rev_results.get("b.txt").unwrap(), "removed");
    }
}
