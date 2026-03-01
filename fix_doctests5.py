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

with open("relvar-core/src/algebra/mod.rs", 'r') as f:
    content = f.read()

# Specifically target the exact lines in the doc block of delta
import re
content = re.sub(r'/// let delta = Delta::between\(&r1, &r2\)\.unwrap\(\);\n', '', content)

with open("relvar-core/src/algebra/mod.rs", 'w') as f:
    f.write(content)
