import re

with open("relvar-storage/src/persistent_engine/mod.rs", "r") as f:
    content = f.read()

content = content.replace(
    """            if self
                .storage_manager
                .read()
                .unwrap()
                .relation_exists(&relation_name)""",
    """            if self
                .storage_manager
                .read()
                .map_err(|e| StorageError::Other(format!("PoisonError: {}", e)))?
                .relation_exists(&relation_name)"""
)

with open("relvar-storage/src/persistent_engine/mod.rs", "w") as f:
    f.write(content)
