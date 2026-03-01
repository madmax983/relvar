import sys

def process_file(path, replacements):
    with open(path, 'r') as f:
        content = f.read()

    for old, new in replacements:
        content = content.replace(old, new)

    with open(path, 'w') as f:
        f.write(content)

process_file("relvar-core/src/algebra/mod.rs", [
    ("use relvar_core::algebra::delta::Delta;", "")
])

process_file("relvar-core/src/algebra/delta.rs", [
    ("use relvar_core::algebra::delta::Delta;", "")
])
