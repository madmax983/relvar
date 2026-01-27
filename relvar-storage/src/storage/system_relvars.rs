//! System catalog relvar definitions.
//!
//! This module defines the structure and names of system relvars that implement
//! the relational system catalog per TTM RM Prescription 12.
//!
//! # System Relvars
//!
//! The system catalog consists of three virtual relvars:
//!
//! - [`SYS_RELVARS`] - Catalog of all relvars (base and virtual)
//! - [`SYS_ATTRIBUTES`] - Catalog of all attributes in all relvars
//! - [`SYS_CONSTRAINTS`] - Catalog of all constraints
//!
//! # TTM Compliance
//!
//! TTM RM Prescription 12: "The entire information content of the database
//! is represented in one and only one way - namely, as explicit values in
//! column positions in rows in tables."
//!
//! System relvars are implemented as virtual relvars (views) that compute their
//! content from the internal metadata structures. This elegantly solves the
//! bootstrap problem - system relvars don't need storage because they ARE
//! computed from the metadata they represent.
//!
//! # Reserved Names
//!
//! All system relvars have names beginning with `SYS_`. User-defined relvars
//! cannot use this prefix to avoid naming conflicts.

use relvar_core::types::{RelationType, ScalarType, TupleType};

/// Name of the system relvar containing metadata about all relvars.
///
/// **Heading:** `{ relvar_name: String, relvar_type: String, heap_file_path: String }`
///
/// **Content:** One tuple for each relvar (base or virtual) in the database.
/// - `relvar_name` - The name of the relvar
/// - `relvar_type` - Either "BASE" or "VIRTUAL"
/// - `heap_file_path` - Path to heap file (empty string for virtual relvars)
pub const SYS_RELVARS: &str = "SYS_RELVARS";

/// Name of the system relvar containing metadata about all attributes.
///
/// **Heading:** `{ relvar_name: String, attr_name: String, attr_type: String, attr_position: Int }`
///
/// **Content:** One tuple for each attribute in each relvar.
/// - `relvar_name` - The name of the relvar containing this attribute
/// - `attr_name` - The name of the attribute
/// - `attr_type` - Serialized type string (e.g., "Int", "String", "UserDefined(WidgetId,Int)")
/// - `attr_position` - Position of attribute in the heading (0-based)
pub const SYS_ATTRIBUTES: &str = "SYS_ATTRIBUTES";

/// Name of the system relvar containing metadata about all constraints.
///
/// **Heading:** `{ relvar_name: String, constraint_name: String, constraint_type: String, constraint_def: String }`
///
/// **Content:** One tuple for each constraint in the database.
/// - `relvar_name` - The name of the relvar to which this constraint applies
/// - `constraint_name` - Unique name for this constraint
/// - `constraint_type` - Type of constraint: "PRIMARY_KEY", "CANDIDATE_KEY", "FOREIGN_KEY", "TYPE"
/// - `constraint_def` - Serialized constraint definition
pub const SYS_CONSTRAINTS: &str = "SYS_CONSTRAINTS";

/// Returns the relation type for the [`SYS_RELVARS`] system relvar.
///
/// **Heading:** `{ relvar_name: String, relvar_type: String, heap_file_path: String }`
///
/// # Example
///
/// ```
/// use relvar::storage::system_relvars::sys_relvars_type;
///
/// let rel_type = sys_relvars_type();
/// assert_eq!(rel_type.degree(), 3);
/// assert!(rel_type.has_attribute("relvar_name"));
/// assert!(rel_type.has_attribute("relvar_type"));
/// assert!(rel_type.has_attribute("heap_file_path"));
/// ```
pub fn sys_relvars_type() -> RelationType {
    let tuple_type = TupleType::new()
        .with_attribute("relvar_name".to_string(), ScalarType::String)
        .with_attribute("relvar_type".to_string(), ScalarType::String)
        .with_attribute("heap_file_path".to_string(), ScalarType::String);
    RelationType::new(tuple_type)
}

/// Returns the relation type for the [`SYS_ATTRIBUTES`] system relvar.
///
/// **Heading:** `{ relvar_name: String, attr_name: String, attr_type: String, attr_position: Int }`
///
/// # Example
///
/// ```
/// use relvar::storage::system_relvars::sys_attributes_type;
///
/// let rel_type = sys_attributes_type();
/// assert_eq!(rel_type.degree(), 4);
/// assert!(rel_type.has_attribute("relvar_name"));
/// assert!(rel_type.has_attribute("attr_name"));
/// assert!(rel_type.has_attribute("attr_type"));
/// assert!(rel_type.has_attribute("attr_position"));
/// ```
pub fn sys_attributes_type() -> RelationType {
    let tuple_type = TupleType::new()
        .with_attribute("relvar_name".to_string(), ScalarType::String)
        .with_attribute("attr_name".to_string(), ScalarType::String)
        .with_attribute("attr_type".to_string(), ScalarType::String)
        .with_attribute("attr_position".to_string(), ScalarType::Int);
    RelationType::new(tuple_type)
}

/// Returns the relation type for the [`SYS_CONSTRAINTS`] system relvar.
///
/// **Heading:** `{ relvar_name: String, constraint_name: String, constraint_type: String, constraint_def: String }`
///
/// # Example
///
/// ```
/// use relvar::storage::system_relvars::sys_constraints_type;
///
/// let rel_type = sys_constraints_type();
/// assert_eq!(rel_type.degree(), 4);
/// assert!(rel_type.has_attribute("relvar_name"));
/// assert!(rel_type.has_attribute("constraint_name"));
/// assert!(rel_type.has_attribute("constraint_type"));
/// assert!(rel_type.has_attribute("constraint_def"));
/// ```
pub fn sys_constraints_type() -> RelationType {
    let tuple_type = TupleType::new()
        .with_attribute("relvar_name".to_string(), ScalarType::String)
        .with_attribute("constraint_name".to_string(), ScalarType::String)
        .with_attribute("constraint_type".to_string(), ScalarType::String)
        .with_attribute("constraint_def".to_string(), ScalarType::String);
    RelationType::new(tuple_type)
}

/// Checks if a relvar name is reserved for system catalog use.
///
/// System relvar names all start with the prefix `SYS_`. This function
/// returns `true` for any name beginning with this prefix.
///
/// # Arguments
///
/// * `name` - The relvar name to check
///
/// # Examples
///
/// ```
/// use relvar::storage::system_relvars::is_system_relvar;
///
/// assert!(is_system_relvar("SYS_RELVARS"));
/// assert!(is_system_relvar("SYS_ATTRIBUTES"));
/// assert!(is_system_relvar("SYS_CUSTOM"));
/// assert!(!is_system_relvar("EMP"));
/// assert!(!is_system_relvar("sys_relvars")); // Case-sensitive
/// ```
pub fn is_system_relvar(name: &str) -> bool {
    name.starts_with("SYS_")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test type definitions
    #[test]
    fn test_sys_relvars_type_has_correct_heading() {
        let rel_type = sys_relvars_type();

        // Should have exactly 3 attributes
        assert_eq!(rel_type.degree(), 3);

        // Check attribute names and types
        assert!(rel_type.has_attribute("relvar_name"));
        assert!(rel_type.has_attribute("relvar_type"));
        assert!(rel_type.has_attribute("heap_file_path"));

        // All attributes should be String type
        let heading = rel_type.heading();
        assert_eq!(
            heading.get_attribute_type("relvar_name"),
            Some(&ScalarType::String)
        );
        assert_eq!(
            heading.get_attribute_type("relvar_type"),
            Some(&ScalarType::String)
        );
        assert_eq!(
            heading.get_attribute_type("heap_file_path"),
            Some(&ScalarType::String)
        );
    }

    #[test]
    fn test_sys_attributes_type_has_correct_heading() {
        let rel_type = sys_attributes_type();

        // Should have exactly 4 attributes
        assert_eq!(rel_type.degree(), 4);

        // Check attribute names
        assert!(rel_type.has_attribute("relvar_name"));
        assert!(rel_type.has_attribute("attr_name"));
        assert!(rel_type.has_attribute("attr_type"));
        assert!(rel_type.has_attribute("attr_position"));

        // Check attribute types
        let heading = rel_type.heading();
        assert_eq!(
            heading.get_attribute_type("relvar_name"),
            Some(&ScalarType::String)
        );
        assert_eq!(
            heading.get_attribute_type("attr_name"),
            Some(&ScalarType::String)
        );
        assert_eq!(
            heading.get_attribute_type("attr_type"),
            Some(&ScalarType::String)
        );
        assert_eq!(
            heading.get_attribute_type("attr_position"),
            Some(&ScalarType::Int)
        );
    }

    #[test]
    fn test_sys_constraints_type_has_correct_heading() {
        let rel_type = sys_constraints_type();

        // Should have exactly 4 attributes
        assert_eq!(rel_type.degree(), 4);

        // Check attribute names
        assert!(rel_type.has_attribute("relvar_name"));
        assert!(rel_type.has_attribute("constraint_name"));
        assert!(rel_type.has_attribute("constraint_type"));
        assert!(rel_type.has_attribute("constraint_def"));

        // Check attribute types (all should be String)
        let heading = rel_type.heading();
        assert_eq!(
            heading.get_attribute_type("relvar_name"),
            Some(&ScalarType::String)
        );
        assert_eq!(
            heading.get_attribute_type("constraint_name"),
            Some(&ScalarType::String)
        );
        assert_eq!(
            heading.get_attribute_type("constraint_type"),
            Some(&ScalarType::String)
        );
        assert_eq!(
            heading.get_attribute_type("constraint_def"),
            Some(&ScalarType::String)
        );
    }

    #[test]
    fn test_is_system_relvar_detects_sys_prefix() {
        // All SYS_ prefixed names should be detected
        assert!(is_system_relvar("SYS_RELVARS"));
        assert!(is_system_relvar("SYS_ATTRIBUTES"));
        assert!(is_system_relvar("SYS_CONSTRAINTS"));
        assert!(is_system_relvar("SYS_CUSTOM"));
        assert!(is_system_relvar("SYS_"));
    }

    #[test]
    fn test_is_system_relvar_case_sensitive() {
        // Should be case-sensitive - lowercase sys_ is not reserved
        assert!(!is_system_relvar("sys_relvars"));
        assert!(!is_system_relvar("Sys_Relvars"));
        assert!(!is_system_relvar("sYs_ReLvArS"));

        // Non-SYS names should not be detected
        assert!(!is_system_relvar("EMP"));
        assert!(!is_system_relvar("EMPLOYEES"));
        assert!(!is_system_relvar("SYS")); // Doesn't have underscore
        assert!(!is_system_relvar("SYSTEM_RELVARS"));
    }

    #[test]
    fn test_system_relvar_constants_are_uppercase() {
        // Verify the constant values match expectations
        assert_eq!(SYS_RELVARS, "SYS_RELVARS");
        assert_eq!(SYS_ATTRIBUTES, "SYS_ATTRIBUTES");
        assert_eq!(SYS_CONSTRAINTS, "SYS_CONSTRAINTS");

        // All constants should be detected as system relvars
        assert!(is_system_relvar(SYS_RELVARS));
        assert!(is_system_relvar(SYS_ATTRIBUTES));
        assert!(is_system_relvar(SYS_CONSTRAINTS));
    }
}
