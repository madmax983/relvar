import sys
import glob

def process_file(path, replacements):
    with open(path, 'r') as f:
        content = f.read()

    for old, new in replacements:
        content = content.replace(old, new)

    with open(path, 'w') as f:
        f.write(content)

process_file("relvar-core/tests/warden_exploit_like.rs", [
    ("use relvar_core::constraints::expression::ConstraintExpression;", "use relvar_core::ConstraintExpression;")
])

process_file("relvar-core/tests/warden_exploit_like_memory.rs", [
    ("use relvar_core::constraints::expression::ConstraintExpression;", "use relvar_core::ConstraintExpression;")
])

process_file("relvar-core/tests/sentry_constraint_recursion.rs", [
    ("use relvar_core::constraints::expression::ConstraintExpression;", "use relvar_core::ConstraintExpression;")
])

files = glob.glob("relvar-core/tests/*.rs")
for path in files:
    process_file(path, [
        ("use relvar_core::constraints::expression::ConstraintExpression;", "use relvar_core::ConstraintExpression;")
    ])
