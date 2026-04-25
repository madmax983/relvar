use crate::database::Database;
use crate::storage_engine::InMemoryEngine;
use crate::types::RelationType;
use crate::types::{ScalarType, TupleType};

/// Helper function to create a basic test relation type.
///
/// # Examples
///
/// ```
/// use relvar_core::database::tests::common::test_rel_type;
/// let rel_type = test_rel_type();
/// assert_eq!(rel_type.degree(), 2);
/// ```
pub fn test_rel_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    )
}

/// Helper function to setup a parent-child database for testing.
///
/// # Examples
///
/// ```
/// use relvar_core::database::tests::common::setup_parent_child_db;
/// let db = setup_parent_child_db();
/// assert!(db.relvar_exists("PARENT"));
/// assert!(db.relvar_exists("CHILD"));
/// ```
pub fn setup_parent_child_db() -> Database<InMemoryEngine> {
    use crate::constraints::{KeyConstraints, PrimaryKey};
    let mut db: Database<InMemoryEngine> = Database::new(InMemoryEngine::new());

    let parent_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    );
    let child_type = RelationType::new(
        TupleType::new()
            .with_attribute("child_id", ScalarType::Int)
            .with_attribute("parent_id", ScalarType::Int),
    );

    db.create_relvar("PARENT", parent_type).unwrap();
    db.create_relvar("CHILD", child_type).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    let parent_constraints = KeyConstraints::new().with_primary_key(pk);
    db.set_key_constraints("PARENT", parent_constraints)
        .unwrap();

    db
}
