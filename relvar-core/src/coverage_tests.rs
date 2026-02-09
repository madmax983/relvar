
use crate::database::Database;
use crate::storage_engine::InMemoryEngine;
use crate::traits::QueryExecutor;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::Relation;
use crate::virtual_relvars::VirtualRelvarDefinition;
use crate::DatabaseError;

#[test]
fn test_query_executor_trait_dispatch() {
    let mut db = Database::new(InMemoryEngine::new());
    let rel_type = RelationType::new(
        TupleType::new().with_attribute("id", ScalarType::Int)
    );
    db.create_relvar("TEST", rel_type.clone()).unwrap();

    // Test dispatch via trait object
    let executor: &mut dyn QueryExecutor = &mut db;

    // Test relvar_exists via trait
    assert!(executor.relvar_exists("TEST"));
    assert!(!executor.relvar_exists("NONEXISTENT"));

    // Test query via trait
    let result = executor.query("TEST");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().cardinality(), 0);
}

#[test]
fn test_virtual_relvar_definition_derived_traits() {
    fn dummy_evaluator(_: &mut dyn QueryExecutor) -> Result<Relation, DatabaseError> {
        Err(DatabaseError::RelationNotFound("dummy".to_string()))
    }

    let def = VirtualRelvarDefinition {
        name: "V1".to_string(),
        relation_type: RelationType::new(TupleType::new()),
        evaluator: dummy_evaluator,
    };

    // Test Clone
    let cloned = def.clone();
    assert_eq!(cloned.name, "V1");

    // Test Debug
    let debug_str = format!("{:?}", def);
    assert!(debug_str.contains("VirtualRelvarDefinition"));
    assert!(debug_str.contains("V1"));
}
