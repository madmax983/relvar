import sys
import glob

files = glob.glob("relvar/src/experimental/*.rs")

for path in files:
    with open(path, 'r') as f:
        content = f.read()

    content = content.replace("use relvar_core::algebra::summarize::Aggregation;", "use relvar_core::algebra::Aggregation;")
    content = content.replace("algebra::summarize::Aggregation,", "algebra::Aggregation,")

    with open(path, 'w') as f:
        f.write(content)
