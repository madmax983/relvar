import sys
import glob

files = glob.glob("relvar/src/experimental/*.rs")

for path in files:
    with open(path, 'r') as f:
        content = f.read()

    content = content.replace("use relvar_core::algebra::Aggregation;", "use relvar_core::Aggregation;")
    content = content.replace("algebra::Aggregation,", "Aggregation,")

    with open(path, 'w') as f:
        f.write(content)
