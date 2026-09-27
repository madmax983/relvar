#![allow(unused_imports)]
use super::common::*;
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use std::collections::HashSet;
use tempfile::NamedTempFile;

#[test]
fn test_update_creates_new_version() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original version
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // Update to new version
    let updated = tuple! { id: 1i64, name: "Updated" };
    let new_tuple_id = heap
        .update_tuple_versioned(tuple_id, &updated, test_txn(2), &mut vpool)
        .unwrap();

    // New version should have different TupleId
    assert_ne!(tuple_id, new_tuple_id);

    // Read new version
    let new_version = heap.read_tuple_versioned(new_tuple_id).unwrap();
    assert_eq!(new_version, updated);
}

#[test]
fn test_update_marks_old_xmax() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original version
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // Update sets xmax on old version
    let updated = tuple! { id: 1i64, name: "Updated" };
    heap.update_tuple_versioned(tuple_id, &updated, test_txn(2), &mut vpool)
        .unwrap();

    // Old version should have xmax set
    let page = heap.load_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmax, Some(test_txn(2)));
}

#[test]
fn test_update_new_version_has_correct_xmin() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original version
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // Update creates new version with updating transaction's ID
    let updated = tuple! { id: 1i64, name: "Updated" };
    let new_tuple_id = heap
        .update_tuple_versioned(tuple_id, &updated, test_txn(2), &mut vpool)
        .unwrap();

    // New version should have xmin = test_txn(2)
    let page = heap.load_page(new_tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[new_tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmin, test_txn(2));
    assert_eq!(slot.xmax, None); // Not yet deleted
}

#[test]
fn test_update_links_versions() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original version
    let original = tuple! { id: 1i64, name: "Original" };
    let old_tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // Update creates version chain
    let updated = tuple! { id: 1i64, name: "Updated" };
    let new_tuple_id = heap
        .update_tuple_versioned(old_tuple_id, &updated, test_txn(2), &mut vpool)
        .unwrap();

    // New version should point back to old version
    let page = heap.load_page(new_tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let new_slot = versioned_page.slots[new_tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(new_slot.prev_version, Some(old_tuple_id));
}

#[test]
fn test_update_concurrent_txn_sees_old_version() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert original version
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // T1 commits
    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);

    // T2: Begin with a real horizon — T3 has not begun yet.
    let snapshot_t2 = fx.begin(test_txn(2), 200).with_horizon(test_txn(3));

    // T3: Update and commit after T2's snapshot.
    fx.begin(test_txn(3), 210);
    let updated = tuple! { id: 1i64, name: "Updated" };
    heap.update_tuple_versioned(tuple_id, &updated, test_txn(3), &mut vpool)
        .unwrap();
    fx.commit(test_txn(3), 220);

    // Snapshot Isolation (commit-LSN tracking in the pool): T3's update began
    // after T2's snapshot, past its horizon, so T2 still sees the old version.
    let visible = fx.scan_visible(&mut heap, &snapshot_t2).unwrap();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], original); // Sees pre-update version (SI semantics)
}

#[test]
fn test_update_updating_txn_sees_new_version() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);

    // T2: Update (uncommitted)
    let snapshot_t2 = fx.begin(test_txn(2), 200);
    let updated = tuple! { id: 1i64, name: "Updated" };
    heap.update_tuple_versioned(tuple_id, &updated, test_txn(2), &mut vpool)
        .unwrap();

    // T2 should see its own update (not yet committed)
    let visible = fx.scan_visible(&mut heap, &snapshot_t2).unwrap();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], updated); // Should see updated version
}

#[test]
fn test_update_multiple_times_creates_chain() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original
    let v1 = tuple! { id: 1i64, name: "V1" };
    let tid1 = heap
        .insert_tuple_versioned(&v1, test_txn(1), &mut vpool)
        .unwrap();

    // Update to V2
    let v2 = tuple! { id: 1i64, name: "V2" };
    let tid2 = heap
        .update_tuple_versioned(tid1, &v2, test_txn(2), &mut vpool)
        .unwrap();

    // Update to V3
    let v3 = tuple! { id: 1i64, name: "V3" };
    let tid3 = heap
        .update_tuple_versioned(tid2, &v3, test_txn(3), &mut vpool)
        .unwrap();

    // Verify chain: tid3 -> tid2 -> tid1
    let page3 = heap.load_page(tid3.page_id).unwrap();
    let vpage3: VersionedSlottedPage = deserialize_versioned_page_for_test(page3.data()).unwrap();
    let slot3 = vpage3.slots[tid3.slot as usize].as_ref().unwrap();
    assert_eq!(slot3.prev_version, Some(tid2));

    let page2 = heap.load_page(tid2.page_id).unwrap();
    let vpage2: VersionedSlottedPage = deserialize_versioned_page_for_test(page2.data()).unwrap();
    let slot2 = vpage2.slots[tid2.slot as usize].as_ref().unwrap();
    assert_eq!(slot2.prev_version, Some(tid1));
}

#[test]
fn test_update_nonexistent_tuple_fails() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let bogus_id = TupleId {
        page_id: 999,
        slot: 0,
    };
    let updated = tuple! { id: 1i64, name: "Updated" };

    let result = heap.update_tuple_versioned(bogus_id, &updated, test_txn(1), &mut vpool);
    assert!(result.is_err());
    assert!(matches!(result, Err(HeapError::TupleNotFound)));
}

#[test]
fn test_update_preserves_tuple_data() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original
    let original = tuple! { id: 42i64, name: "OriginalData" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // Update with different data
    let updated = tuple! { id: 42i64, name: "UpdatedData" };
    let new_tuple_id = heap
        .update_tuple_versioned(tuple_id, &updated, test_txn(2), &mut vpool)
        .unwrap();

    // Verify both versions have correct data
    let old_tuple = heap.read_tuple_versioned(tuple_id).unwrap();
    assert_eq!(old_tuple, original);

    let new_tuple = heap.read_tuple_versioned(new_tuple_id).unwrap();
    assert_eq!(new_tuple, updated);
}

#[test]
fn test_update_old_version_xmin_unchanged() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // T2: Update
    let updated = tuple! { id: 1i64, name: "Updated" };
    heap.update_tuple_versioned(tuple_id, &updated, test_txn(2), &mut vpool)
        .unwrap();

    // Old version should still have xmin = test_txn(1)
    let page = heap.load_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmin, test_txn(1));
}

#[test]
fn test_update_different_pages() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert original
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // Update might go to different page if original page is full
    let updated = tuple! { id: 1i64, name: "Updated" };
    let new_tuple_id = heap
        .update_tuple_versioned(tuple_id, &updated, test_txn(2), &mut vpool)
        .unwrap();

    // Both versions should be readable
    let old_tuple = heap.read_tuple_versioned(tuple_id).unwrap();
    assert_eq!(old_tuple, original);

    let new_tuple = heap.read_tuple_versioned(new_tuple_id).unwrap();
    assert_eq!(new_tuple, updated);
}

#[test]
fn test_update_after_commit_visible_to_later_txn() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);

    // T2: Update and commit
    fx.begin(test_txn(2), 150);
    let updated = tuple! { id: 1i64, name: "Updated" };
    heap.update_tuple_versioned(tuple_id, &updated, test_txn(2), &mut vpool)
        .unwrap();
    fx.commit(test_txn(2), 200);

    // T3: Should see updated version
    let snapshot_t3 = fx.begin(test_txn(3), 300);
    let visible = fx.scan_visible(&mut heap, &snapshot_t3).unwrap();

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], updated);
}

// Phase 5.2: Delete Marks xmax (TDD - RED)

#[test]
fn test_heap_update_on_corrupted_page_fails() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // 1. Insert a tuple to get a valid TupleId
    let tuple = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    // 2. Corrupt the page (slot pointing outside)
    // We need to read the page, construct a corrupted version, and write it back
    {
        let page = heap.load_page(tuple_id.page_id).unwrap();

        // Deserialize (valid)
        let _versioned_page: VersionedSlottedPage =
            deserialize_versioned_page_for_test(page.data()).unwrap();

        // Create corrupted version
        let corrupted_page = VersionedSlottedPage {
            magic: VERSIONED_PAGE_MAGIC,
            slot_count: 1,
            slots: vec![Some(VersionedSlotEntry {
                offset: (PAGE_SIZE + 100) as u32, // Invalid offset
                length: 10,
                xmin: test_txn(1),
                xmax: None,
                prev_version: None,
            })],
        };

        // Serialize and write back
        let slot_dir = postcard::to_allocvec(&corrupted_page).unwrap();
        let mut page_data = vec![0u8; PAGE_SIZE - 8];
        page_data[0] = PAGE_FORMAT_VERSION;
        let slot_dir_len = slot_dir.len() as u32;
        page_data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());
        page_data[5..5 + slot_dir.len()].copy_from_slice(&slot_dir);

        let new_page = Page::from_data(tuple_id.page_id, page_data).unwrap();
        heap.store_page(&new_page).unwrap();
    }

    // 3. Try to update the tuple
    // This should fail when extracting existing tuples in update_tuple_versioned
    let updated = tuple! { id: 1i64, name: "Updated" };
    let result = heap.update_tuple_versioned(tuple_id, &updated, test_txn(2), &mut vpool);

    // 4. Assert failure
    assert!(result.is_err(), "Update should fail on corrupted page");
    match result {
        Err(HeapError::Slotted(SlottedError::Serialization(msg))) => {
            assert!(
                msg.contains("Corrupted slot") || msg.contains("outside page data"),
                "Unexpected error message: {}",
                msg
            );
        }
        _ => panic!("Expected Slotted error, got {:?}", result),
    }
}

#[test]
fn test_gc_removes_entire_update_chain() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert
    let v1 = tuple! { id: 1i64, name: "V1" };
    let tid1 = heap
        .insert_tuple_versioned(&v1, test_txn(1), &mut vpool)
        .unwrap();

    // T2: Update to V2
    let v2 = tuple! { id: 1i64, name: "V2" };
    let tid2 = heap
        .update_tuple_versioned(tid1, &v2, test_txn(2), &mut vpool)
        .unwrap();

    // T3: Update to V3
    let v3 = tuple! { id: 1i64, name: "V3" };
    let _tid3 = heap
        .update_tuple_versioned(tid2, &v3, test_txn(3), &mut vpool)
        .unwrap();

    let mut committed = HashSet::new();
    committed.insert(test_txn(1));
    committed.insert(test_txn(2));
    committed.insert(test_txn(3));

    // All transactions committed; V2's and V3's updaters committed at
    // LSNs 200 and 300, before the oldest active LSN.
    let txn_pool = pool_with_commits(&[(1, 100), (2, 200), (3, 300)]);
    let oldest_active = test_lsn(400);

    // Should remove V1 and V2 (both have xmax and are old)
    let removed = heap
        .gc_remove_dead_versions(oldest_active, &committed, &txn_pool, &mut vpool)
        .unwrap();

    assert_eq!(removed, 2);
}

#[test]
fn test_failed_update_rolls_back_xmax_marking() {
    // RED: a replacement-insert failure must not leave the old version
    // marked deleted. The update writes the xmax-marked old page first;
    // if the new version's page write then fails, the xmax marking has to
    // be undone so the failed update mutates nothing.
    let (device, handle) = FailAfterDevice::new();
    let mut heap = HeapFile::create_on_device(device, create_test_relation_type());
    let mut vpool = test_version_pool();

    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // Arm the device to fail the update's second page write: the update
    // stores the xmax-marked old page, then stores the new version's page.
    // Failing that second store exercises the rollback path.
    handle.fail_after(handle.writes() + 1);

    let updated = tuple! { id: 1i64, name: "Updated" };
    let result = heap.update_tuple_versioned(tuple_id, &updated, test_txn(2), &mut vpool);
    assert!(
        result.is_err(),
        "update should fail on the injected device error"
    );

    // The old version's xmax must be rolled back to None: the failed
    // update leaves the database unchanged.
    let page = heap.load_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();
    assert_eq!(
        slot.xmax, None,
        "failed update left the old version marked deleted"
    );

    // And no new version may exist: the page still holds exactly the one
    // original slot.
    assert_eq!(versioned_page.slot_count, 1);

    // The version-pool claim is released as well.
    assert_eq!(vpool.release_for_txn(test_txn(2)), 0);
}

#[test]
fn test_double_failure_update_reports_atomicity_loss() {
    // RED: when the replacement insert fails AND the compensating rollback
    // of the old version's xmax marking also fails, the update must not
    // swallow the rollback error. The old version stays marked deleted by
    // this transaction, so the caller has to abort: committing would hide
    // the old version without its replacement, silently losing the tuple.
    // The typed error carries both failures instead of discarding one.
    let (device, handle) = FailAfterDevice::new();
    let mut heap = HeapFile::create_on_device(device, create_test_relation_type());
    let mut vpool = test_version_pool();

    let original = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // Let the xmax-marking store succeed, then fail the next two page
    // writes: the new version's page (insert fails) and the rollback's
    // restore of the old page (rollback fails).
    handle.skip_then_fail_next(1, 2);

    let updated = tuple! { id: 1i64, name: "Updated" };
    let result = heap.update_tuple_versioned(tuple_id, &updated, test_txn(2), &mut vpool);
    let Err(HeapError::UpdateRollbackFailed {
        insert_error,
        rollback_error,
    }) = result
    else {
        panic!("double failure must surface UpdateRollbackFailed, got {result:?}");
    };
    assert!(
        insert_error.contains("injected write failure"),
        "insert error should name the device failure, got: {insert_error}"
    );
    assert!(
        rollback_error.contains("injected write failure"),
        "rollback error should name the device failure, got: {rollback_error}"
    );

    // The residual state is honest: the old version is still marked
    // deleted by the failed transaction (the rollback did not land).
    let page = heap.load_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();
    assert_eq!(
        slot.xmax,
        Some(test_txn(2)),
        "failed rollback must leave the xmax marking in place, visibly"
    );

    // The version-pool claim is still released: the pool is capacity
    // accounting, not page state, so it recycles normally.
    assert_eq!(vpool.release_for_txn(test_txn(2)), 0);
}
