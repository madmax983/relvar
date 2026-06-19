## 2023-10-27 - Split HeapFile Monolith
**Tangle:** The `relvar-storage/src/storage/heap/mod.rs` file grew to over 1500 lines, containing `HeapError`, `Page` layouts, and the `HeapFile` implementation, violating the "Blob" anti-pattern.
**Blueprint:** Refactored the module into smaller submodules (`error.rs`, `page.rs`, `heap_file.rs`) and used `mod.rs` as a facade to export the public API, enforcing better structural boundaries.
