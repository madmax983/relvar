import re

with open('relvar/src/lib.rs', 'r') as f:
    content = f.read()

new_content = content.replace("pub(crate) mod experimental;", "pub mod experimental;")
new_content = new_content.replace("pub(crate) mod tools;", "pub mod tools;")

with open('relvar/src/lib.rs', 'w') as f:
    f.write(new_content)
