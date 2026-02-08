use relvar::{Database, InMemoryEngine};
use relvar::types::{RelationType, ScalarType, TupleType};
use relvar::experimental::system_views;
use relvar::constraints::{KeyConstraints, PrimaryKey};

#[test]
fn test_register_system_views() {
    let mut db = Database::new(InMemoryEngine::new());
    system_views::register(&mut db).unwrap();

    assert!(db.relvar_exists("_RELVARS"));
    assert!(db.relvar_exists("_ATTRIBUTES"));
}

#[test]
fn test_query_relvars() {
    let mut db = Database::new(InMemoryEngine::new());
    system_views::register(&mut db).unwrap();

    // Create a user relvar
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
    );
    db.create_relvar("USERS", rel_type).unwrap();

    // Query _RELVARS
    let relvars = db.query("_RELVARS").unwrap();

    // Should contain USERS, _RELVARS, and _ATTRIBUTES
    assert!(relvars.cardinality() >= 3);

    // Verify USERS
    let users_tuple = relvars.tuples().find(|t|
        t.get_typed::<String>("name").unwrap() == "USERS"
    ).unwrap();

    assert_eq!(users_tuple.get_typed::<i64>("degree").unwrap(), 2);

    // Verify _RELVARS itself
    let meta_tuple = relvars.tuples().find(|t|
        t.get_typed::<String>("name").unwrap() == "_RELVARS"
    ).unwrap();
    assert_eq!(meta_tuple.get_typed::<i64>("degree").unwrap(), 2);
}

#[test]
fn test_query_attributes_with_pk() {
    let mut db = Database::new(InMemoryEngine::new());
    system_views::register(&mut db).unwrap();

    // Create relvar with PK
    let rel_type = RelationType::new(
        TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("email", ScalarType::String)
    );
    db.create_relvar("USERS", rel_type).unwrap();

    let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
    db.set_key_constraints("USERS", KeyConstraints::new().with_primary_key(pk)).unwrap();

    // Query _ATTRIBUTES
    let attrs = db.query("_ATTRIBUTES").unwrap();

    // Filter for USERS
    let user_attrs = attrs.restrict(|t|
        t.get_typed::<String>("relvar_name").unwrap() == "USERS"
    );

    assert_eq!(user_attrs.cardinality(), 2);

    // Check id (PK)
    let id_attr = user_attrs.tuples().find(|t|
        t.get_typed::<String>("attribute_name").unwrap() == "id"
    ).unwrap();
    assert_eq!(id_attr.get_typed::<String>("type").unwrap(), "Int");
    assert!(id_attr.get_typed::<bool>("is_pk").unwrap());

    // Check email (not PK)
    let email_attr = user_attrs.tuples().find(|t|
        t.get_typed::<String>("attribute_name").unwrap() == "email"
    ).unwrap();
    assert_eq!(email_attr.get_typed::<String>("type").unwrap(), "String");
    assert!(!email_attr.get_typed::<bool>("is_pk").unwrap());
}

#[test]
fn test_dynamic_updates() {
    let mut db = Database::new(InMemoryEngine::new());
    system_views::register(&mut db).unwrap();

    // Initially empty (except system views)
    let count_before = db.query("_RELVARS").unwrap().cardinality();

    // Add new relvar
    let rel_type = RelationType::new(TupleType::new().with_attribute("x", ScalarType::Int));
    db.create_relvar("NEW_REL", rel_type).unwrap();

    // Query again
    let count_after = db.query("_RELVARS").unwrap().cardinality();
    assert_eq!(count_after, count_before + 1);

    let relvars = db.query("_RELVARS").unwrap();
    assert!(relvars.tuples().any(|t| t.get_typed::<String>("name").unwrap() == "NEW_REL"));
}
