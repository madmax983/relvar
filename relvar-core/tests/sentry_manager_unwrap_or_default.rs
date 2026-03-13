use relvar_core::constraints::ConstraintManager;
use relvar_core::storage_engine::InMemoryEngine;
use relvar_core::tuple;

#[test]
fn test_validate_foreign_keys_single_tuple_no_constraints() {
    let mut engine = InMemoryEngine::new();
    let manager = ConstraintManager::new();

    // We haven't added any constraints to manager

    let tuple = tuple! { id: 1i64 };

    // validate_foreign_keys_single_tuple uses unwrap_or_default()
    // Let's ensure it doesn't fail when no constraints exist.
    let res = manager.validate_foreign_keys_single_tuple(&mut engine, "USERS", &tuple);
    assert!(res.is_ok());
}
