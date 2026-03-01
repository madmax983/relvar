import sys

def process_file(path, replacements):
    with open(path, 'r') as f:
        content = f.read()

    for old, new in replacements:
        content = content.replace(old, new)

    with open(path, 'w') as f:
        f.write(content)

process_file("relvar/src/lib.rs", [
    ("pub(crate) mod experimental;", "pub mod experimental;"),
    ("pub(crate) mod tools;", "pub mod tools;")
])

process_file("relvar-storage/src/lib.rs", [
    ("pub(crate) mod persistent_engine;", "pub mod persistent_engine;"),
    ("pub(crate) mod storage;", "pub mod storage;")
])
