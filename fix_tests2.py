import sys
import glob

def process_file(path, replacements):
    with open(path, 'r') as f:
        content = f.read()

    for old, new in replacements:
        content = content.replace(old, new)

    with open(path, 'w') as f:
        f.write(content)

files = glob.glob("relvar-core/tests/*.rs")
for path in files:
    process_file(path, [
        ("use relvar_core::constraints::expression::{CmpOp, ConstraintExpression, ValueOrRef};", "use relvar_core::{CmpOp, ConstraintExpression, ValueOrRef};")
    ])
