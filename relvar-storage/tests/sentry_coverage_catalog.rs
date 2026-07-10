use relvar_storage::storage::{Catalog, CatalogError};
use std::fs::File;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_catalog_load_too_large() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut file = File::create(path).unwrap();
    file.write_all(b"{\"relations\":{").unwrap();
    for _ in 0..10 * 1024 {
        file.write_all(&[b' '; 1024]).unwrap(); // 10MB
    }
    file.write_all(b"}}").unwrap();
    file.flush().unwrap();

    let result = Catalog::load(path);
    assert!(result.is_err());
    match result {
        Err(CatalogError::Serialization(msg)) => {
            assert!(msg.contains("too large"));
        }
        _ => panic!("Expected Serialization error"),
    }
}
