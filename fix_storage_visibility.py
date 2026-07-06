with open("relvar-storage/src/lib.rs", "r") as f:
    text = f.read()

# restore pub mod for persistent_engine, storage, mvcc, wal
text = text.replace("pub(crate) mod persistent_engine;", "pub mod persistent_engine;")
text = text.replace("pub(crate) mod storage;", "pub mod storage;")
text = text.replace("pub(crate) mod mvcc;", "pub mod mvcc;")
text = text.replace("pub(crate) mod wal;", "pub mod wal;")

with open("relvar-storage/src/lib.rs", "w") as f:
    f.write(text)
