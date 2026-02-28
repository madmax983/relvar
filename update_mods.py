import re
import os

def process_file(path, replacements):
    with open(path, 'r') as f:
        content = f.read()

    for old, new in replacements:
        content = content.replace(old, new)

    with open(path, 'w') as f:
        f.write(content)

# relvar-core
process_file("relvar-core/src/lib.rs", [
    ("pub mod algebra;", "pub(crate) mod algebra;"),
    ("pub mod constraints;", "pub(crate) mod constraints;"),
    ("pub mod database;", "pub(crate) mod database;"),
    ("pub mod error;", "pub(crate) mod error;"),
    ("pub mod query;", "pub(crate) mod query;"),
    ("pub mod storage_engine;", "pub(crate) mod storage_engine;"),
    ("pub mod traits;", "pub(crate) mod traits;"),
    ("pub mod types;", "pub(crate) mod types;"),
    ("pub mod values;", "pub(crate) mod values;"),
    ("pub use storage_engine::{InMemoryEngine, StorageEngine, StorageError};",
     "pub use storage_engine::{InMemoryEngine, StorageEngine, StorageError, RelationMetadata};")
])

# relvar-storage
process_file("relvar-storage/src/persistent_engine.rs", [
    ("use relvar_core::storage_engine::{RelationMetadata, StorageEngine, StorageError};",
     "use relvar_core::{RelationMetadata, StorageEngine, StorageError};"),
    ("use relvar_core::types::RelationType;", "use relvar_core::RelationType;"),
    ("use relvar_core::values::{Relation, Tuple};", "use relvar_core::{Relation, Tuple};")
])

process_file("relvar-storage/src/storage/catalog.rs", [
    ("use relvar_core::types::RelationType;", "use relvar_core::RelationType;")
])

process_file("relvar-storage/src/storage/heap.rs", [
    ("use relvar_core::types::RelationType;", "use relvar_core::RelationType;"),
    ("use relvar_core::values::{Relation, Tuple};", "use relvar_core::{Relation, Tuple};")
])

process_file("relvar-storage/src/storage/manager.rs", [
    ("use relvar_core::storage_engine::{RelationMetadata, StorageError};",
     "use relvar_core::{RelationMetadata, StorageError};"),
    ("use relvar_core::types::RelationType;", "use relvar_core::RelationType;"),
    ("use relvar_core::values::{Relation, Tuple};", "use relvar_core::{Relation, Tuple};")
])
