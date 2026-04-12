import re

with open("relvar-storage/src/mvcc/gc.rs", "r") as f:
    content = f.read()

test_mod = content[content.find("#[cfg(test)]\nmod tests {"):]

with open("relvar-storage/src/storage/heap.rs", "r") as f:
    heap_content = f.read()

# Replace collect_garbage with heap.gc_remove_dead_versions in tests
test_mod = test_mod.replace("collect_garbage(&mut heap, oldest_active, &committed)", "heap.gc_remove_dead_versions(oldest_active, &committed)")

# In the test mod, we don't need `use crate::storage::heap::HeapFile;` and `use super::*;` can stay but we should adapt it since it will be inside heap.rs
# Actually, the tests are already in `relvar-storage/src/storage/heap.rs`? Let's check
