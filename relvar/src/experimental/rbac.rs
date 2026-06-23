//! Relational Role-Based Access Control (RBAC)
//!
//! Models a full RBAC system with role hierarchies using pure relational algebra.
//!
//! # Concept
//!
//! Access control systems often need to resolve complex role hierarchies to determine
//! a user's effective permissions. Using relational algebra, this recursive problem
//! becomes a declarative data transformation:
//!
//! - **Transitive Closure**: We use `tclose` on the role hierarchy to find all ancestor roles.
//! - **Set Union**: We combine direct roles with inherited roles.
//! - **Natural Join**: We join the expanded roles with the permissions relation to get
//!   effective permissions.
//!
//! # Schema
//! - **UserRoles**: `(user_id: String, role_id: String)`
//! - **RoleHierarchy**: `(role_id: String, inherits_from: String)`
//! - **Permissions**: `(role_id: String, resource: String, action: String)`

use relvar_core::{error::DatabaseError, values::Relation};

/// A Relational RBAC Engine.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::RelationalRbac;
/// // Note: This is a placeholder example
/// ```
pub struct RelationalRbac {
    /// Users and their directly assigned roles. Schema: (user_id: String, role_id: String)
    pub user_roles: Relation,
    /// Role inheritance tree. Schema: (role_id: String, inherits_from: String)
    pub role_hierarchy: Relation,
    /// Permissions assigned to roles. Schema: (role_id: String, resource: String, action: String)
    pub permissions: Relation,
}

impl RelationalRbac {
    /// Creates a new Relational RBAC Engine.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::RelationalRbac;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(user_roles: Relation, role_hierarchy: Relation, permissions: Relation) -> Self {
        Self {
            user_roles,
            role_hierarchy,
            permissions,
        }
    }

    /// Computes the effective permissions for all users.
    ///
    /// Returns a relation with schema: (user_id: String, resource: String, action: String)
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::RelationalRbac;
    /// // Note: This is a placeholder example
    /// ```
    pub fn effective_permissions(&self) -> Result<Relation, DatabaseError> {
        // 1. Compute all inherited roles using transitive closure
        let inherited = if self.role_hierarchy.cardinality() > 0 {
            self.role_hierarchy.tclose("role_id", "inherits_from")?
        } else {
            self.role_hierarchy.clone()
        };

        // 2. Expand user roles by joining with inherited roles
        let user_inherited_roles = self.user_roles.join(&inherited)?;

        let user_inherited_roles_proj = user_inherited_roles
            .project(&["user_id", "inherits_from"])
            .rename(&[("inherits_from", "role_id")]);

        // 3. Combine direct roles with inherited roles
        let all_user_roles = self
            .user_roles
            .union(&user_inherited_roles_proj)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Join with permissions to get the effective access rights
        let effective = all_user_roles.join(&self.permissions)?;

        // 5. Project out the role_id to deduplicate overlapping permissions from different roles
        Ok(effective.project(&["user_id", "resource", "action"]))
    }

    /// Checks if a specific user has permission to perform an action on a resource.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::RelationalRbac;
    /// // Note: This is a placeholder example
    /// ```
    pub fn check_access(
        &self,
        user_id: &str,
        resource: &str,
        action: &str,
    ) -> Result<bool, DatabaseError> {
        let permissions = self.effective_permissions()?;

        // Relational restrict to find matches
        let matches = permissions.restrict(|t| {
            let u = t.get_typed::<String>("user_id").unwrap();
            let r = t.get_typed::<String>("resource").unwrap();
            let a = t.get_typed::<String>("action").unwrap();
            u == user_id && r == resource && a == action
        });

        Ok(matches.cardinality() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_rbac_inheritance() {
        let ur_heading = TupleType::new()
            .with_attribute("user_id", ScalarType::String)
            .with_attribute("role_id", ScalarType::String);
        let mut user_roles = Relation::new(RelationType::new(ur_heading));
        user_roles
            .insert(tuple! { user_id: "alice", role_id: "admin" })
            .unwrap();
        user_roles
            .insert(tuple! { user_id: "bob", role_id: "editor" })
            .unwrap();
        user_roles
            .insert(tuple! { user_id: "charlie", role_id: "viewer" })
            .unwrap();

        let rh_heading = TupleType::new()
            .with_attribute("role_id", ScalarType::String)
            .with_attribute("inherits_from", ScalarType::String);
        let mut role_hierarchy = Relation::new(RelationType::new(rh_heading));
        // admin inherits from editor
        role_hierarchy
            .insert(tuple! { role_id: "admin", inherits_from: "editor" })
            .unwrap();
        // editor inherits from viewer
        role_hierarchy
            .insert(tuple! { role_id: "editor", inherits_from: "viewer" })
            .unwrap();

        let p_heading = TupleType::new()
            .with_attribute("role_id", ScalarType::String)
            .with_attribute("resource", ScalarType::String)
            .with_attribute("action", ScalarType::String);
        let mut permissions = Relation::new(RelationType::new(p_heading));
        permissions
            .insert(tuple! { role_id: "viewer", resource: "article", action: "read" })
            .unwrap();
        permissions
            .insert(tuple! { role_id: "editor", resource: "article", action: "write" })
            .unwrap();
        permissions
            .insert(tuple! { role_id: "admin", resource: "system", action: "config" })
            .unwrap();

        let rbac = RelationalRbac::new(user_roles, role_hierarchy, permissions);

        // Charlie (viewer)
        assert!(rbac.check_access("charlie", "article", "read").unwrap());
        assert!(!rbac.check_access("charlie", "article", "write").unwrap());

        // Bob (editor) - should inherit read
        assert!(rbac.check_access("bob", "article", "write").unwrap());
        assert!(rbac.check_access("bob", "article", "read").unwrap());
        assert!(!rbac.check_access("bob", "system", "config").unwrap());

        // Alice (admin) - should inherit write and read
        assert!(rbac.check_access("alice", "system", "config").unwrap());
        assert!(rbac.check_access("alice", "article", "write").unwrap());
        assert!(rbac.check_access("alice", "article", "read").unwrap());

        // Get all effective permissions
        let effective = rbac.effective_permissions().unwrap();
        // Admin gets 3, Editor gets 2, Viewer gets 1 = 6 total tuples
        assert_eq!(effective.cardinality(), 6);
    }
}
