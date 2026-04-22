use super::common::*;
use crate::persistent_engine::*;
use relvar_core::tuple;
use tempfile::TempDir;

#[test]
fn test_create_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 1);
}

#[test]
fn test_persistence() {
    let temp_dir = TempDir::new().unwrap();

    // Create database and insert data
    {
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
        engine.create_relation("TEST", test_rel_type()).unwrap();
        engine
            .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
    }

    // Reopen database and verify data persists
    {
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();
        assert!(engine.relation_exists("TEST"));

        let relation = engine.load_relation("TEST").unwrap();
        assert_eq!(relation.cardinality(), 1);
    }
}

#[test]
fn test_path_traversal_protection() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    // Try to create relation with path separator
    let result = engine.create_relation("../evil", test_rel_type());
    assert!(result.is_err());

    // Try with backslash
    let result = engine.create_relation("..\\evil", test_rel_type());
    assert!(result.is_err());

    // Try with forward slash
    let result = engine.create_relation("sub/dir", test_rel_type());
    assert!(result.is_err());

    // Try with parent directory reference
    let result = engine.create_relation("..", test_rel_type());
    assert!(result.is_err());

    // Try with empty name
    let result = engine.create_relation("", test_rel_type());
    assert!(result.is_err());

    // Valid name should work
    let result = engine.create_relation("VALID_NAME", test_rel_type());
    assert!(result.is_ok());
}

#[test]
fn test_drop_relation() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    assert!(engine.relation_exists("TEST"));

    // Drop the relation
    engine.drop_relation("TEST").unwrap();

    assert!(!engine.relation_exists("TEST"));

    // Dropping again should fail
    let result = engine.drop_relation("TEST");
    assert!(result.is_err());
}

#[test]
fn test_store_relation_empty() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // Insert some data
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
        .unwrap();

    // Store empty relation
    let empty_relation = Relation::new(test_rel_type());
    engine.store_relation("TEST", &empty_relation).unwrap();

    // Verify relation is now empty
    let loaded = engine.load_relation("TEST").unwrap();
    assert_eq!(loaded.cardinality(), 0);
}

#[test]
fn test_store_relation_large() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // Create relation with many tuples
    let mut large_relation = Relation::new(test_rel_type());
    for i in 0..100 {
        large_relation
            .insert(tuple! { id: i as i64, name: format!("Name{}", i) })
            .unwrap();
    }

    engine.store_relation("TEST", &large_relation).unwrap();

    // Verify all tuples persisted
    let loaded = engine.load_relation("TEST").unwrap();
    assert_eq!(loaded.cardinality(), 100);
}

#[test]
fn test_list_relations() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    // Initially empty
    assert_eq!(engine.list_relations().len(), 0);

    // Create some relations
    engine.create_relation("REL1", test_rel_type()).unwrap();
    engine.create_relation("REL2", test_rel_type()).unwrap();
    engine.create_relation("REL3", test_rel_type()).unwrap();

    let relations = engine.list_relations();
    assert_eq!(relations.len(), 3);
    assert!(relations.contains(&"REL1".to_string()));
    assert!(relations.contains(&"REL2".to_string()));
    assert!(relations.contains(&"REL3".to_string()));
}

#[test]
fn test_get_relation_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    let metadata = engine.get_relation_metadata("TEST").unwrap();
    assert_eq!(metadata.name, "TEST");
    assert_eq!(metadata.relation_type.degree(), 2);
    assert!(metadata.relation_type.heading().has_attribute("id"));
    assert!(metadata.relation_type.heading().has_attribute("name"));
}

#[test]
fn test_insert_tuple_nonexistent_relation() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    let result = engine.insert_tuple("NONEXISTENT", tuple! { id: 1i64, name: "Alice" });
    assert!(result.is_err());
}

#[test]
fn test_load_relation_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let engine = PersistentEngine::open(temp_dir.path()).unwrap();

    let result = engine.load_relation("NONEXISTENT");
    assert!(result.is_err());
}

#[test]
fn test_duplicate_relation_name() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // Creating again should fail
    let result = engine.create_relation("TEST", test_rel_type());
    assert!(result.is_err());
}

#[test]
fn test_reopen_with_existing_relations() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple relations
    {
        let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();
        engine.create_relation("REL1", test_rel_type()).unwrap();
        engine.create_relation("REL2", test_rel_type()).unwrap();
        engine
            .insert_tuple("REL1", tuple! { id: 1i64, name: "Alice" })
            .unwrap();
        engine
            .insert_tuple("REL2", tuple! { id: 2i64, name: "Bob" })
            .unwrap();
    }

    // Reopen and verify both relations exist
    {
        let engine = PersistentEngine::open(temp_dir.path()).unwrap();
        assert_eq!(engine.list_relations().len(), 2);

        let rel1 = engine.load_relation("REL1").unwrap();
        assert_eq!(rel1.cardinality(), 1);

        let rel2 = engine.load_relation("REL2").unwrap();
        assert_eq!(rel2.cardinality(), 1);
    }
}

#[test]
fn test_multiple_inserts_uses_cached_heap_file() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // Multiple inserts should reuse the cached heap file
    for i in 0..10 {
        engine
            .insert_tuple("TEST", tuple! { id: i as i64, name: format!("Name{}", i) })
            .unwrap();
    }

    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 10);
}

#[test]
fn test_store_relation_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    let relation = Relation::new(test_rel_type());
    let result = engine.store_relation("NONEXISTENT", &relation);
    assert!(result.is_err());
}

#[test]
fn test_get_metadata_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let engine = PersistentEngine::open(temp_dir.path()).unwrap();

    let result = engine.get_relation_metadata("NONEXISTENT");
    assert!(result.is_err());
}

#[test]
fn test_drop_nonexistent_relation() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    let result = engine.drop_relation("NONEXISTENT");
    assert!(result.is_err());
}

#[test]
fn test_store_relation_replaces_data() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();

    // Insert initial data
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
        .unwrap();

    // Create new relation with different data
    let mut new_relation = Relation::new(test_rel_type());
    new_relation
        .insert(tuple! { id: 100i64, name: "Charlie" })
        .unwrap();

    // Store should replace old data
    engine.store_relation("TEST", &new_relation).unwrap();

    let loaded = engine.load_relation("TEST").unwrap();
    assert_eq!(loaded.cardinality(), 1);
    assert!(loaded.contains(&tuple! { id: 100i64, name: "Charlie" }));
}

#[test]
fn test_relation_exists_returns_false_for_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let engine = PersistentEngine::open(temp_dir.path()).unwrap();

    assert!(!engine.relation_exists("NONEXISTENT"));
}

#[test]
fn test_relation_exists_returns_true_for_existing() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();
    assert!(engine.relation_exists("TEST"));
}

#[test]
fn test_empty_database_list_relations() {
    let temp_dir = TempDir::new().unwrap();
    let engine = PersistentEngine::open(temp_dir.path()).unwrap();

    assert_eq!(engine.list_relations().len(), 0);
}

#[test]
fn test_insert_then_load_uses_cached_file() {
    let temp_dir = TempDir::new().unwrap();
    let mut engine = PersistentEngine::open(temp_dir.path()).unwrap();

    engine.create_relation("TEST", test_rel_type()).unwrap();
    engine
        .insert_tuple("TEST", tuple! { id: 1i64, name: "Alice" })
        .unwrap();

    // Load should work with cached heap file
    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 1);

    // Insert more using cache
    engine
        .insert_tuple("TEST", tuple! { id: 2i64, name: "Bob" })
        .unwrap();

    let relation = engine.load_relation("TEST").unwrap();
    assert_eq!(relation.cardinality(), 2);
}

// WAL Integration Tests (Step 5)
