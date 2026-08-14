with open('relvar/src/lib.rs', 'r') as f:
    content = f.read()

search = """
/// Developer tools and utilities.
pub(crate) mod tools;
"""

replace = """
/// Developer tools and utilities.
pub mod tools;
"""

content = content.replace(search[1:], replace[1:])
with open('relvar/src/lib.rs', 'w') as f:
    f.write(content)
