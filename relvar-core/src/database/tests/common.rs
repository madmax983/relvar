use crate::database::Database;
use crate::storage_engine::InMemoryEngine;
use crate::types::RelationType;
use crate::types::{ScalarType, TupleType};

pub(crate) fn test_rel_type() -> RelationType {
    RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String),
    )
}

pub(crate) fn setup_parent_child_db() -> Database<InMemoryEngine> {
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
