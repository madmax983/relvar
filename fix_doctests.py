import sys
import glob

def process_file(path, replacements):
    with open(path, 'r') as f:
        content = f.read()

    for old, new in replacements:
        content = content.replace(old, new)

    with open(path, 'w') as f:
        f.write(content)

files = glob.glob("relvar-core/src/algebra/*.rs")
for path in files:
    process_file(path, [
        ("use relvar_core::algebra::summarize::Aggregation;", "use relvar_core::algebra::Aggregation;")
    ])

process_file("relvar-core/src/algebra/mod.rs", [
    ("use relvar_core::algebra::delta::Delta;", ""),
    ("use relvar_core::algebra::summarize::Aggregation;", "use relvar_core::algebra::Aggregation;")
])

process_file("relvar-core/src/constraints/check.rs", [
    ("use relvar_core::constraints::expression::{ConstraintExpression, CmpOp, ValueOrRef};", "use relvar_core::{ConstraintExpression, CmpOp, ValueOrRef};")
])

process_file("relvar-core/src/constraints/expression.rs", [
    ("use relvar_core::constraints::expression::{ConstraintExpression, CmpOp, ValueOrRef};", "use relvar_core::{ConstraintExpression, CmpOp, ValueOrRef};"),
    ("use relvar_core::constraints::expression::ConstraintExpression;", "use relvar_core::ConstraintExpression;")
])
