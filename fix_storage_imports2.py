import sys

filepath = 'relvar-storage/src/lib.rs'
with open(filepath, 'r') as f:
    content = f.read()

content = content.replace(
    'pub(crate) mod storage;',
    'pub mod storage;'
)
content = content.replace(
    'pub(crate) mod persistent_engine;',
    'pub mod persistent_engine;'
)
content = content.replace(
    'pub(crate) mod mvcc;',
    'pub mod mvcc;'
)
content = content.replace(
    'pub(crate) mod wal;',
    'pub mod wal;'
)


with open(filepath, 'w') as f:
    f.write(content)
