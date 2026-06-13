use super::*;
use tempfile::NamedTempFile;

#[test]
fn test_page_creation() {
    let page = Page::new(0);
    assert_eq!(page.id(), 0);
    assert!(page.is_empty());
    assert_eq!(page.available_space(), PAGE_SIZE);
}

#[test]
fn test_page_from_data() {
    let data = vec![1, 2, 3, 4, 5];
    let page = Page::from_data(42, data.clone()).unwrap();

    assert_eq!(page.id(), 42);
    assert_eq!(page.data(), &data);
    assert_eq!(page.available_space(), PAGE_SIZE - 5);
}

#[test]
fn test_page_too_large() {
    let data = vec![0u8; PAGE_SIZE + 1];
    let result = Page::from_data(0, data);

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PageError::PageTooLarge));
}

#[test]
fn test_page_set_data() {
    let mut page = Page::new(0);
    let data = vec![1, 2, 3];

    page.set_data(data.clone()).unwrap();
    assert_eq!(page.data(), &data);
}

#[test]
fn test_page_file_write_and_read() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Write a page
    {
        let mut page_file = PageFile::create(path).unwrap();
        let data = vec![1, 2, 3, 4, 5];
        let page = Page::from_data(0, data.clone()).unwrap();

        page_file.write_page(&page).unwrap();
    }

    // Read it back
    {
        let mut page_file = PageFile::open(path).unwrap();
        let page = page_file.read_page(0).unwrap();

        assert_eq!(page.id(), 0);
        assert_eq!(page.data(), &[1, 2, 3, 4, 5]);
    }
}

#[test]
fn test_page_file_multiple_pages() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Write multiple pages
    {
        let mut page_file = PageFile::create(path).unwrap();

        for i in 0..5 {
            let data = vec![i as u8; 10];
            let page = Page::from_data(i, data).unwrap();
            page_file.write_page(&page).unwrap();
        }
    }

    // Read them back
    {
        let mut page_file = PageFile::open(path).unwrap();

        for i in 0..5 {
            let page = page_file.read_page(i).unwrap();
            assert_eq!(page.id(), i);
            assert_eq!(page.data().len(), 10);
            assert!(page.data().iter().all(|&b| b == i as u8));
        }
    }
}

#[test]
fn test_page_file_empty_page() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut page_file = PageFile::create(path).unwrap();

    // Write an empty page
    let page = Page::new(0);
    page_file.write_page(&page).unwrap();

    // Read it back
    let read_page = page_file.read_page(0).unwrap();
    assert_eq!(read_page.id(), 0);
    assert!(read_page.is_empty());
}

#[test]
fn test_page_file_overwrite() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut page_file = PageFile::create(path).unwrap();

    // Write initial data
    let page1 = Page::from_data(0, vec![1, 2, 3]).unwrap();
    page_file.write_page(&page1).unwrap();

    // Overwrite with new data
    let page2 = Page::from_data(0, vec![4, 5, 6, 7]).unwrap();
    page_file.write_page(&page2).unwrap();

    // Read it back
    let read_page = page_file.read_page(0).unwrap();
    assert_eq!(read_page.data(), &[4, 5, 6, 7]);
}

#[test]
fn test_write_buffered_no_immediate_sync() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_path_buf();

    {
        let mut page_file = PageFile::create(&path).unwrap();
        let page = Page::from_data(0, vec![1, 2, 3, 4, 5]).unwrap();

        // Write buffered - data may not be on disk yet
        page_file.write_page_buffered(&page).unwrap();

        // Note: We can't reliably test that data ISN'T on disk because
        // the OS may flush buffers at any time. This test just verifies
        // the method succeeds.
    }

    // After closing and reopening, data should be there (OS flushes on close)
    {
        let mut page_file = PageFile::open(&path).unwrap();
        let read_page = page_file.read_page(0).unwrap();
        assert_eq!(read_page.data(), &[1, 2, 3, 4, 5]);
    }
}

#[test]
fn test_explicit_sync_persists_data() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_path_buf();

    {
        let mut page_file = PageFile::create(&path).unwrap();
        let page = Page::from_data(0, vec![42, 43, 44]).unwrap();

        // Write buffered then explicitly sync
        page_file.write_page_buffered(&page).unwrap();
        page_file.sync().unwrap();
    }

    // Data should survive reopen
    {
        let mut page_file = PageFile::open(&path).unwrap();
        let read_page = page_file.read_page(0).unwrap();
        assert_eq!(read_page.data(), &[42, 43, 44]);
    }
}

#[test]
fn test_write_then_crash_simulation() {
    // This test demonstrates that without sync, data MIGHT be lost
    // (though in practice, OS buffering makes this hard to test reliably)
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_path_buf();

    {
        let mut page_file = PageFile::create(&path).unwrap();
        let page = Page::from_data(0, vec![99, 98, 97]).unwrap();

        // Write buffered without sync
        page_file.write_page_buffered(&page).unwrap();

        // Simulate crash by dropping file handle without calling sync
        // (Note: The OS may still flush on drop, so this is not a perfect test)
        drop(page_file);
    }

    // In a real crash scenario, this data might be lost. But for testing
    // purposes, we just verify the API works correctly. A true crash
    // recovery test would require actual power-off simulation.
    // For now, we just verify the file can be opened and read.
    let mut page_file = PageFile::open(&path).unwrap();
    let _read_page = page_file.read_page(0);
    // Don't assert on data content - might or might not be there
}

#[test]
fn test_page_file_corrupted_length() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut page_file = PageFile::create(path).unwrap();

    // manually write a page with corrupted length
    // Length 9999 (larger than PAGE_SIZE)
    let bad_len: u64 = 9999;
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&bad_len.to_le_bytes());
    buffer.resize(PAGE_SIZE, 0); // Fill rest with zeros

    // Write manually to file
    {
        let mut file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        use std::io::Write;
        file.write_all(&buffer).unwrap();
    }

    // Now read it back - should error
    let result = page_file.read_page(0);
    assert!(result.is_err());
    match result {
        Err(PageError::Serialization(msg)) => {
            // 9999 > 4088 (PAGE_SIZE-8), so it hits the max size check first
            assert!(msg.contains("exceeds maximum"));
        }
        _ => panic!("Expected Serialization error"),
    }
}

#[test]
fn test_page_file_too_short() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut page_file = PageFile::create(path).unwrap();

    // manually write a partial page (less than 8 bytes)
    let data = vec![1u8, 2, 3];

    // Write manually to file
    {
        let mut file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        use std::io::Write;
        file.write_all(&data).unwrap();
    }

    // Now read it back - should error
    let result = page_file.read_page(0);
    assert!(result.is_err());
    match result {
        Err(PageError::Serialization(msg)) => {
            assert!(msg.contains("Page too short"));
        }
        _ => panic!("Expected Serialization error"),
    }
}

#[test]
fn test_page_file_data_len_overflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut page_file = PageFile::create(path).unwrap();

    // manually write a page with data_len = u64::MAX
    // This simulates a corrupted/malicious page
    let bad_len: u64 = u64::MAX;
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&bad_len.to_le_bytes());
    buffer.resize(PAGE_SIZE, 0); // Fill rest with zeros

    // Write manually to file
    {
        let mut file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        use std::io::Write;
        file.write_all(&buffer).unwrap();
    }

    // Now read it back - should error cleanly without panic
    let result = page_file.read_page(0);
    assert!(result.is_err());
    match result {
        Err(PageError::Serialization(msg)) => {
            // u64::MAX > PAGE_SIZE-8, so it hits the max size check first
            assert!(
                msg.contains("exceeds maximum"),
                "Unexpected message: {}",
                msg
            );
        }
        _ => panic!(
            "Expected Serialization error with overflow message, got {:?}",
            result
        ),
    }
}

#[test]
fn test_page_file_length_boundary() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut page_file = PageFile::create(path).unwrap();
    let max_data = PAGE_SIZE - 8;

    // Test Case 1: Max allowed length (PAGE_SIZE - 8)
    {
        let len = max_data as u64;
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&len.to_le_bytes());
        buffer.resize(PAGE_SIZE, 0); // Fill with valid data

        // Write directly
        let mut file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        use std::io::Seek;
        use std::io::SeekFrom;
        use std::io::Write;
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&buffer).unwrap();
    }

    // Should succeed
    let result = page_file.read_page(0);
    assert!(result.is_ok());
    let page = result.unwrap();
    assert_eq!(page.data().len(), max_data);

    // Test Case 2: Max allowed length + 1 (PAGE_SIZE - 7)
    {
        let len = (max_data + 1) as u64;
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&len.to_le_bytes());
        buffer.resize(PAGE_SIZE, 0);

        // Write directly
        let mut file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        use std::io::Seek;
        use std::io::SeekFrom;
        use std::io::Write;
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&buffer).unwrap();
    }

    // Should fail
    let result = page_file.read_page(0);
    assert!(result.is_err());
    match result {
        Err(PageError::Serialization(msg)) => {
            assert!(msg.contains("Page data length"), "Unexpected: {}", msg);
            assert!(msg.contains("exceeds maximum"), "Unexpected: {}", msg);
        }
        _ => panic!("Expected Serialization error, got {:?}", result),
    }

    // Test Case 3: 4GB + 1 (Simulate 32-bit truncation vulnerability)
    // 0x100000001 = 4294967297
    // On 32-bit, this casts to 1. If we don't check u64 first, it might pass.
    {
        let len: u64 = 0x100000001;
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&len.to_le_bytes());
        buffer.resize(PAGE_SIZE, 0);

        // Write directly
        let mut file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        use std::io::Seek;
        use std::io::SeekFrom;
        use std::io::Write;
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&buffer).unwrap();
    }

    // Should fail cleanly with our new check
    let result = page_file.read_page(0);
    assert!(result.is_err());
    match result {
        Err(PageError::Serialization(msg)) => {
            assert!(msg.contains("Page data length"), "Unexpected: {}", msg);
            assert!(msg.contains("exceeds maximum"), "Unexpected: {}", msg);
            // Verify it actually printed the huge number, not the truncated one
            assert!(
                msg.contains("4294967297"),
                "Did not detect full u64 length: {}",
                msg
            );
        }
        _ => panic!("Expected Serialization error, got {:?}", result),
    }
}
