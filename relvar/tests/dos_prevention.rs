use relvar::tuple;
use relvar::types::{RelationType, ScalarType, TupleType};
use relvar::{Database, PersistentEngine};
use tempfile::TempDir;

#[test]
fn test_insert_huge_tuple_fails_gracefully() {
    let temp_dir = TempDir::new().unwrap();
    let mut db = Database::new(PersistentEngine::open(temp_dir.path()).unwrap());

    let rel_type = RelationType::new(TupleType::new().with_attribute("data", ScalarType::Bytes));

    db.create_relvar("TEST", rel_type).unwrap();

    // 1. Try to insert a tuple larger than PAGE_SIZE (4096)
    // overhead is ~40-60 bytes. 5000 bytes is definitely too large.
    let huge_data = vec![0u8; 5000];
    let tuple = tuple! { data: huge_data };

    let result = db.insert("TEST", tuple);

    // Should fail
    assert!(result.is_err());

    // Verify that the invalid tuple was NOT logged to WAL (DoS prevention)
    let wal_path = temp_dir.path().join("wal.log");
    let wal_size = std::fs::metadata(&wal_path).unwrap().len();

    // WAL should contain:
    // - Magic number (8 bytes)
    // - Create relation records (small)
    // - Maybe Begin transaction record (small)
    // 5000 bytes tuple would definitely increase this significantly if logged.
    // We expect size to be small.
    // Let's print it to be sure.
    println!("WAL Size: {}", wal_size);

    assert!(
        wal_size < 4000,
        "WAL size {} is too large! Invalid tuple was likely logged.",
        wal_size
    );
}
