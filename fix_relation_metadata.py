import sys

def process_file(path, replacements):
    with open(path, 'r') as f:
        content = f.read()

    for old, new in replacements:
        content = content.replace(old, new)

    with open(path, 'w') as f:
        f.write(content)

process_file("relvar-core/src/lib.rs", [
    ("pub use storage_engine::{InMemoryEngine, StorageEngine, StorageError};",
     "pub use storage_engine::{InMemoryEngine, StorageEngine, StorageError, RelationMetadata};")
])
