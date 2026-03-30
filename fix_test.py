with open("relvar-core/tests/sentry_database_integrity_coverage.rs", "r") as f:
    content = f.read()

content = content.replace("""    // Test that missing keys gracefully handle
    assert!(db.get_foreign_key_constraints("TEST").is_none());""", """    // Test that missing keys gracefully handle
    assert!(db.get_foreign_key_constraints("TEST").is_none());

    // Also missing key_constraints for relation that doesn't exist
    assert!(db.get_key_constraints("NONEXISTENT").is_none());
    assert!(db.get_foreign_key_constraints("NONEXISTENT").is_none());""")

with open("relvar-core/tests/sentry_database_integrity_coverage.rs", "w") as f:
    f.write(content)
