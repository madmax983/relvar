import sys

filepath = 'relvar/tests/sentry_timeseries_collision.rs'
with open(filepath, 'r') as f:
    content = f.read()

content = content.replace(
    '|t: &Tuple| t.get_typed::<i64>("time") == Some(2)',
    '|t| t.get_typed::<i64>("time") == Some(2)'
)

with open(filepath, 'w') as f:
    f.write(content)
