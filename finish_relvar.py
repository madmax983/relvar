import sys
import glob

def process_file(path, replacements):
    with open(path, 'r') as f:
        content = f.read()

    for old, new in replacements:
        content = content.replace(old, new)

    with open(path, 'w') as f:
        f.write(content)

process_file("relvar/src/lib.rs", [
    ("pub mod experimental;", "pub(crate) mod experimental;"),
    ("pub mod tools;", "pub(crate) mod tools;")
])

process_file("relvar-storage/src/lib.rs", [
    ("pub mod persistent_engine;", "pub(crate) mod persistent_engine;"),
    ("pub mod storage;", "pub(crate) mod storage;")
])
