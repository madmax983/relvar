import sys
import glob

def process_file(path, replacements):
    with open(path, 'r') as f:
        content = f.read()

    for old, new in replacements:
        content = content.replace(old, new)

    with open(path, 'w') as f:
        f.write(content)

files = glob.glob("relvar-core/benches/*.rs")
for path in files:
    process_file(path, [
        ("use relvar_core::algebra::summarize::Aggregation;", "use relvar_core::algebra::Aggregation;")
    ])
