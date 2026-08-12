import sys

filepath = 'relvar/src/lib.rs'
with open(filepath, 'r') as f:
    content = f.read()

content = content.replace(
    'pub(crate) mod experimental;',
    'pub mod experimental;'
)
content = content.replace(
    'pub(crate) mod tools;',
    'pub mod tools;'
)

with open(filepath, 'w') as f:
    f.write(content)
