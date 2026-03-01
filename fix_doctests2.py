import sys

def process_file(path, replacements):
    with open(path, 'r') as f:
        content = f.read()

    for old, new in replacements:
        content = content.replace(old, new)

    with open(path, 'w') as f:
        f.write(content)

process_file("relvar-core/src/constraints/check.rs", [
    ("use relvar_core::constraints::check::CheckConstraint;", "use relvar_core::CheckConstraint;")
])

process_file("relvar-core/src/algebra/mod.rs", [
    ("use relvar_core::algebra::delta::Delta;", "")
])
