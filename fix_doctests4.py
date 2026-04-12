import re

with open("relvar-storage/src/lib.rs", "r") as f:
    content = f.read()

content = content.replace("pub mod wal;", "pub(crate) mod wal;")
content = content.replace("pub mod mvcc;", "pub(crate) mod mvcc;")

with open("relvar-storage/src/lib.rs", "w") as f:
    f.write(content)

with open("relvar-storage/src/storage/mod.rs", "r") as f:
    content = f.read()

content = content.replace("pub mod heap;", "pub(crate) mod heap;")

with open("relvar-storage/src/storage/mod.rs", "w") as f:
    f.write(content)

with open("relvar-storage/src/mvcc/mod.rs", "r") as f:
    content = f.read()

content = content.replace("pub use visibility::{VersionMetadata, is_visible};", "pub use visibility::VersionMetadata;")

with open("relvar-storage/src/mvcc/mod.rs", "w") as f:
    f.write(content)

