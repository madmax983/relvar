with open('relvar/src/lib.rs', 'r') as f:
    content = f.read()

search = """
/// Experimental features that may be unstable or subject to change.
pub(crate) mod experimental;
"""

replace = """
/// Experimental features that may be unstable or subject to change.
pub mod experimental;
"""

content = content.replace(search[1:], replace[1:])
with open('relvar/src/lib.rs', 'w') as f:
    f.write(content)
