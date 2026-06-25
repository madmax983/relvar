sed -i 's/self.offset += 8;/self.offset = self.offset.checked_add(8).expect("Offset overflow");/' relvar-storage/src/wal/iter.rs
sed -i 's/\*slot_count += 1;/*slot_count = slot_count.checked_add(1).expect("Slot count overflow");/' relvar-storage/src/storage/heap/mod.rs
sed -i 's/page_id += 1;/page_id = page_id.checked_add(1).expect("PageId overflow");/' relvar-storage/src/storage/heap/mod.rs
sed -i 's/removed_count += self.gc_process_page(page_id, &page, oldest_active_lsn, committed)?;/removed_count = removed_count.checked_add(self.gc_process_page(page_id, \&page, oldest_active_lsn, committed)?).expect("removed_count overflow");/' relvar-storage/src/storage/heap/mod.rs
sed -i 's/removed_count += 1;/removed_count = removed_count.checked_add(1).expect("removed_count overflow");/' relvar-storage/src/storage/heap/mod.rs
