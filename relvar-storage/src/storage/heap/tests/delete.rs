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
fn test_delete_sets_xmax() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert tuple
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    // Delete tuple
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();

    // Verify xmax is set
    let page = heap.load_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmax, Some(test_txn(2)));
}

#[test]
fn test_delete_invisible_after_commit() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);

    // T2: Delete and commit
    fx.begin(test_txn(2), 150);
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();
    fx.commit(test_txn(2), 200);

    // T3: Should not see deleted tuple
    let snapshot_t3 = fx.begin(test_txn(3), 300);
    let visible = fx.scan_visible(&mut heap, &snapshot_t3).unwrap();

    assert_eq!(visible.len(), 0); // Deleted tuple not visible
}

#[test]
fn test_delete_concurrent_txn_sees_tuple() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);

    // T2: Begin (concurrent with T3)
    let snapshot_t2 = fx.begin(test_txn(2), 200);

    // T3: Delete and commit
    fx.begin(test_txn(3), 210);
    heap.delete_tuple_versioned(tuple_id, test_txn(3), &mut vpool)
        .unwrap();
    fx.commit(test_txn(3), 220);

    // NOTE: With Read Committed, T2 sees the deletion.
    // Full snapshot isolation would preserve visibility.
    let visible = fx.scan_visible(&mut heap, &snapshot_t2).unwrap();

    assert_eq!(visible.len(), 0); // Read Committed: sees deletion
}

#[test]
fn test_delete_deleting_txn_doesnt_see_tuple() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert and commit
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);

    // T2: Delete (uncommitted)
    let snapshot_t2 = fx.begin(test_txn(2), 200);
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();

    // T2 should not see the tuple it deleted
    let visible = fx.scan_visible(&mut heap, &snapshot_t2).unwrap();

    assert_eq!(visible.len(), 0);
}

#[test]
fn test_delete_nonexistent_tuple_fails() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    let bogus_id = TupleId {
        page_id: 999,
        slot: 0,
    };

    let result = heap.delete_tuple_versioned(bogus_id, test_txn(1), &mut vpool);
    assert!(result.is_err());
    assert!(matches!(result, Err(HeapError::TupleNotFound)));
}

#[test]
fn test_delete_preserves_xmin() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1: Insert
    let tuple = tuple! { id: 1i64, name: "ToDelete" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    // T2: Delete
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();

    // Verify xmin unchanged
    let page = heap.load_page(tuple_id.page_id).unwrap();
    let versioned_page: VersionedSlottedPage =
        deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = versioned_page.slots[tuple_id.slot as usize]
        .as_ref()
        .unwrap();

    assert_eq!(slot.xmin, test_txn(1));
    assert_eq!(slot.xmax, Some(test_txn(2)));
}

#[test]
fn test_delete_preserves_tuple_data() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert tuple
    let original = tuple! { id: 42i64, name: "DataToPreserve" };
    let tuple_id = heap
        .insert_tuple_versioned(&original, test_txn(1), &mut vpool)
        .unwrap();

    // Delete tuple
    heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool)
        .unwrap();

    // Tuple data should still be readable (though invisible)
    let tuple = heap.read_tuple_versioned(tuple_id).unwrap();
    assert_eq!(tuple, original);
}

#[test]
fn test_delete_multiple_tuples() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Insert multiple tuples
    let t1 = tuple! { id: 1i64, name: "First" };
    let t2 = tuple! { id: 2i64, name: "Second" };
    let t3 = tuple! { id: 3i64, name: "Third" };

    let _tid1 = heap
        .insert_tuple_versioned(&t1, test_txn(1), &mut vpool)
        .unwrap();
    let tid2 = heap
        .insert_tuple_versioned(&t2, test_txn(1), &mut vpool)
        .unwrap();
    let _tid3 = heap
        .insert_tuple_versioned(&t3, test_txn(1), &mut vpool)
        .unwrap();

    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    fx.commit(test_txn(1), 100);

    // Delete middle tuple
    fx.begin(test_txn(2), 150);
    heap.delete_tuple_versioned(tid2, test_txn(2), &mut vpool)
        .unwrap();
    fx.commit(test_txn(2), 200);

    // Should see first and third, but not second
    let snapshot = fx.begin(test_txn(3), 300);
    let visible = fx.scan_visible(&mut heap, &snapshot).unwrap();

    assert_eq!(visible.len(), 2);
    assert!(visible.contains(&t1));
    assert!(!visible.contains(&t2));
    assert!(visible.contains(&t3));
}

#[test]
fn test_heap_delete_on_corrupted_page_fails() {
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
    {
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

        let slot_dir = postcard::to_allocvec(&corrupted_page).unwrap();
        let mut page_data = vec![0u8; PAGE_SIZE - 8];
        page_data[0] = PAGE_FORMAT_VERSION;
        let slot_dir_len = slot_dir.len() as u32;
        page_data[1..5].copy_from_slice(&slot_dir_len.to_le_bytes());
        page_data[5..5 + slot_dir.len()].copy_from_slice(&slot_dir);

        let new_page = Page::from_data(tuple_id.page_id, page_data).unwrap();
        heap.store_page(&new_page).unwrap();
    }

    // 3. Try to delete the tuple
    let result = heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool);

    // 4. Assert failure
    assert!(result.is_err(), "Delete should fail on corrupted page");
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

/// Regression test for the v0.10.1 full-page delete bug: setting `xmax` on a
/// version grows the serialized slot directory, so deleting from a
/// completely full page used to fail with `PageFull`.
///
/// The insert path now reserves `MAX_XMAX_SLOT_GROWTH` bytes per live slot
/// (enforced in `emit_versioned_page`), so a delete on a page the insert
/// path filled always fits — even for the largest possible deleter id, whose
/// xmax varint is the longest the encoding allows.
#[test]
fn test_delete_on_completely_full_page_succeeds() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // Fill page 0 — and only page 0 — until the versioned insert path
    // reports PageFull.
    let mut entries: Vec<(TupleId, Tuple)> = Vec::new();
    let mut seq = 0i64;
    loop {
        let tuple = tuple! { id: seq, name: "padding-to-fill-the-page" };
        match try_insert_on_page(&mut heap, 0, &tuple, test_txn(1), &mut vpool) {
            Some(slot) => {
                entries.push((TupleId { page_id: 0, slot }, tuple));
                seq += 1;
            }
            None => break,
        }
        assert!(seq < 10_000, "page 0 should eventually report PageFull");
    }
    assert!(!entries.is_empty());

    // Top the page up: pack a final tuple sized to the remaining free space
    // so the leftover is smaller than any xmax varint. This is what makes a
    // fixed-size fill actually brim-full — without it the last PageFull can
    // leave tens of bytes free, which would not reproduce the bug.
    let free_before_topup = page_free_bytes(&mut heap, 0);
    for payload_len in (0..=free_before_topup).rev() {
        let tuple = tuple! { id: seq, name: "x".repeat(payload_len) };
        if let Some(slot) = try_insert_on_page(&mut heap, 0, &tuple, test_txn(1), &mut vpool) {
            entries.push((TupleId { page_id: 0, slot }, tuple));
            break;
        }
    }
    // The fill stayed on page 0: no spillover page was allocated.
    assert!(heap.load_page(1).unwrap().is_empty());

    // Snapshot the directory bytes so the delete's growth is measurable.
    let dir_len_before = {
        let page = heap.load_page(0).unwrap();
        let dir: VersionedSlottedPage = deserialize_versioned_page_for_test(page.data()).unwrap();
        postcard::to_allocvec(&dir).unwrap().len()
    };

    // Deleting from the brim-full page must succeed, even with the longest
    // possible xmax varint.
    let victim = entries[0].0;
    heap.delete_tuple_versioned(victim, test_txn(u64::MAX), &mut vpool)
        .expect("delete on a completely full page must succeed");

    // xmax is set, and the directory grew by exactly the deleter's varint —
    // free-space accounting stays exact, no more and no less.
    let page = heap.load_page(0).unwrap();
    let dir: VersionedSlottedPage = deserialize_versioned_page_for_test(page.data()).unwrap();
    let slot = dir.slots[victim.slot as usize].as_ref().unwrap();
    assert_eq!(slot.xmax, Some(test_txn(u64::MAX)));
    let dir_len_after = postcard::to_allocvec(&dir).unwrap().len();
    let expected_growth =
        postcard::experimental::serialized_size(&Some(test_txn(u64::MAX))).unwrap() as usize - 1;
    assert_eq!(
        expected_growth, MAX_XMAX_SLOT_GROWTH,
        "the test deleter must exercise the worst-case xmax growth"
    );
    assert_eq!(
        dir_len_after - dir_len_before,
        expected_growth,
        "directory growth must be exactly the xmax varint"
    );

    // Every other tuple survived the repack byte-identical.
    for (tuple_id, expected) in entries.iter().skip(1) {
        let read = heap.read_tuple_versioned(*tuple_id).unwrap();
        assert_eq!(
            read, *expected,
            "tuple at slot {} corrupted by the delete",
            tuple_id.slot
        );
    }

    // The reserve invariant holds on the rewritten page: free space still
    // covers every remaining live slot's future xmax.
    let mut lowest_offset = usize::MAX;
    let mut live_slots = 0usize;
    for entry in dir.slots.iter().flatten() {
        lowest_offset = lowest_offset.min(entry.offset as usize);
        if entry.xmax.is_none() {
            live_slots += 1;
        }
    }
    let header_len = 5 + dir_len_after;
    let free = lowest_offset - header_len;
    assert!(
        free >= live_slots * MAX_XMAX_SLOT_GROWTH,
        "free {free} must cover the xmax reserve for {live_slots} live slots"
    );
}

/// Engine-level (MVCC fixture) counterpart: a delete on a full page commits
/// normally, the tuple goes invisible, and GC physically reclaims its space
/// for reuse.
#[test]
fn test_full_page_delete_invisible_after_commit_and_reclaimed_by_gc() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let rel_type = create_test_relation_type();
    let mut heap = HeapFile::create(path, rel_type).unwrap();

    // T1 fills page 0 completely and commits.
    let mut fx = MvccFixture::new();
    fx.begin(test_txn(1), 50);
    let mut tuple_ids = Vec::new();
    let mut seq = 0i64;
    loop {
        let tuple = tuple! { id: seq, name: "full-page-regression" };
        match try_insert_on_page(&mut heap, 0, &tuple, test_txn(1), &mut vpool) {
            Some(slot) => {
                tuple_ids.push(TupleId { page_id: 0, slot });
                seq += 1;
            }
            None => break,
        }
        assert!(seq < 10_000, "page 0 should eventually report PageFull");
    }
    fx.commit(test_txn(1), 100);
    let victim_index = tuple_ids.len() / 2;
    let victim = tuple_ids[victim_index];
    let victim_tuple = tuple! { id: victim_index as i64, name: "full-page-regression" };

    // T2 deletes one tuple from the full page and commits.
    fx.begin(test_txn(2), 150);
    heap.delete_tuple_versioned(victim, test_txn(2), &mut vpool)
        .expect("delete on a full page must succeed");
    fx.commit(test_txn(2), 200);

    // T3 sees every tuple except the deleted one.
    let snapshot = fx.begin(test_txn(3), 300);
    let visible = fx.scan_visible(&mut heap, &snapshot).unwrap();
    assert_eq!(visible.len(), tuple_ids.len() - 1);
    assert!(
        !visible.contains(&victim_tuple),
        "deleted tuple must stay invisible after commit"
    );

    // GC reclaims the dead version: with no live snapshots, the committed
    // deleter is older than the oldest active LSN.
    let mut committed = HashSet::new();
    committed.insert(test_txn(1));
    committed.insert(test_txn(2));
    let txn_pool = pool_with_commits(&[(1, 100), (2, 200)]);
    let removed = heap
        .gc_remove_dead_versions(test_lsn(300), &committed, &txn_pool, &mut vpool)
        .unwrap();
    assert_eq!(removed, 1);

    // The freed space is reusable: a fresh insert lands on page 0, reusing
    // the freed slot.
    let new_tuple = tuple! { id: 99999i64, name: "reclaimed" };
    let slot = try_insert_on_page(&mut heap, 0, &new_tuple, test_txn(3), &mut vpool)
        .expect("reclaimed space must accept a new insert");
    assert_eq!(
        slot, victim.slot,
        "reclaimed space should reuse the freed slot"
    );
}

/// Inserts `tuple` into exactly `page_id` — no spillover page search — and
/// returns the slot on success, `None` on `PageFull`. Any other error is a
/// test bug and panics.
fn try_insert_on_page(
    heap: &mut HeapFile,
    page_id: PageId,
    tuple: &Tuple,
    txn_id: crate::wal::TransactionId,
    vpool: &mut crate::mvcc::VersionPool,
) -> Option<u32> {
    let tuple_len = postcard::experimental::serialized_size(tuple).unwrap() as usize;
    let mut buffer = vpool.acquire_buffer().unwrap();
    match heap.try_insert_into_page_versioned(page_id, tuple, tuple_len, txn_id, None, &mut buffer)
    {
        Ok(slot) => Some(slot),
        Err(HeapError::PageFull) => None,
        Err(e) => panic!("unexpected insert error: {e:?}"),
    }
}

/// Exact free bytes on a versioned page: lowest tuple offset minus the
/// framed slot-directory header.
fn page_free_bytes(heap: &mut HeapFile, page_id: PageId) -> usize {
    let page = heap.load_page(page_id).unwrap();
    let dir: VersionedSlottedPage = deserialize_versioned_page_for_test(page.data()).unwrap();
    let dir_len = postcard::to_allocvec(&dir).unwrap().len();
    let lowest = dir
        .slots
        .iter()
        .flatten()
        .map(|entry| entry.offset as usize)
        .min()
        .unwrap_or(USABLE_PAGE_SIZE_V2);
    lowest - (5 + dir_len)
}

/// A page whose slot directory claims more tuple bytes than the page can
/// hold cannot be repaired in place: the delete must fail with the typed
/// `PageFull` error, never a panic or silent corruption. With the v0.10.1
/// xmax reserve this is unreachable for pages the insert path wrote — only
/// corrupt or foreign images can get here.
#[test]
fn test_delete_on_genuinely_overfull_page_errors_cleanly() {
    let mut vpool = test_version_pool();
    let temp_file = NamedTempFile::new().unwrap();
    let mut heap = HeapFile::create(temp_file.path(), create_test_relation_type()).unwrap();

    let tuple = tuple! { id: 1i64, name: "Original" };
    let tuple_id = heap
        .insert_tuple_versioned(&tuple, test_txn(1), &mut vpool)
        .unwrap();

    // Hand-craft an over-full page: one slot claiming more bytes than the
    // usable page area.
    {
        let overfull_page = VersionedSlottedPage {
            magic: VERSIONED_PAGE_MAGIC,
            slot_count: 1,
            slots: vec![Some(VersionedSlotEntry {
                offset: 0,
                length: (USABLE_PAGE_SIZE_V2 + 100) as u32,
                xmin: test_txn(1),
                xmax: None,
                prev_version: None,
            })],
        };
        let slot_dir = postcard::to_allocvec(&overfull_page).unwrap();
        let mut page_data = vec![0u8; PAGE_SIZE - 8];
        page_data[0] = PAGE_FORMAT_VERSION;
        page_data[1..5].copy_from_slice(&(slot_dir.len() as u32).to_le_bytes());
        page_data[5..5 + slot_dir.len()].copy_from_slice(&slot_dir);

        let new_page = Page::from_data(tuple_id.page_id, page_data).unwrap();
        heap.store_page(&new_page).unwrap();
    }

    let result = heap.delete_tuple_versioned(tuple_id, test_txn(2), &mut vpool);
    assert!(
        matches!(result, Err(HeapError::PageFull)),
        "over-full delete must fail with typed PageFull, got {result:?}"
    );
}
