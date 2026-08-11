sed -i 's/pub(crate) mod experimental;/pub mod experimental;/g' relvar/src/lib.rs
sed -i 's/pub(crate) mod tools;/pub mod tools;/g' relvar/src/lib.rs
sed -i 's/pub(crate) mod persistent_engine;/pub mod persistent_engine;/g' relvar-storage/src/lib.rs
sed -i 's/pub(crate) mod storage;/pub mod storage;/g' relvar-storage/src/lib.rs
sed -i 's/pub(crate) mod wal;/pub mod wal;/g' relvar-storage/src/lib.rs
sed -i 's/pub(crate) mod mvcc;/pub mod mvcc;/g' relvar-storage/src/lib.rs
