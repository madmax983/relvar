import sys

filepath = 'relvar/tests/sentry_timeseries_collision.rs'
with open(filepath, 'r') as f:
    content = f.read()

content = content.replace(
    'use relvar_core::{Database, RelationType, ScalarType, TupleType};',
    'use relvar_core::{Database, RelationType, ScalarType, TupleType, Tuple};'
)

with open(filepath, 'w') as f:
    f.write(content)
