import sys

def process_file(path, replacements):
    with open(path, 'r') as f:
        content = f.read()

    for old, new in replacements:
        content = content.replace(old, new)

    with open(path, 'w') as f:
        f.write(content)

process_file("relvar-core/src/algebra/mod.rs", [
    ("let delta = Delta::between(&r1, &r2).unwrap();", ""),
    ("/// use relvar_core::algebra::delta::Delta;", ""),
    ("/// let delta = Delta::between(&r1, &r2).unwrap();", "")
])
