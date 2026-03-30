import re

def process_file(filepath, replacements):
    with open(filepath, 'r') as f:
        content = f.read()

    for old, new in replacements:
        if old in content:
            content = content.replace(old, new)
        else:
            print(f"Warning: Could not find '{old}' in {filepath}")

    with open(filepath, 'w') as f:
        f.write(content)

process_file("relvar-storage/src/storage/catalog.rs", [
    (
        "/// - `relation_type` - The type (heading) defining attribute names and types\n/// - `heap_file_path` - Path to the heap file containing the relation's tuples\n/// Metadata for a stored relation (relvar).",
        "/// - `relation_type` - The type (heading) defining attribute names and types\n/// - `heap_file_path` - Path to the heap file containing the relation's tuples\n///\n/// Metadata for a stored relation (relvar)."
    )
])
