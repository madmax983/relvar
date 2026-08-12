import sys

filepath = 'relvar/tests/sentry_timeseries_collision.rs'
with open(filepath, 'r') as f:
    content = f.read()

content = content.replace(
    'use relvar::{Relation, RelationType, ScalarType, TupleType};',
    'use relvar::{Relation, RelationType, ScalarType, TupleType, Tuple};'
)

with open(filepath, 'w') as f:
    f.write(content)
