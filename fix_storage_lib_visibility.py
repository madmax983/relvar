with open('relvar-storage/src/lib.rs', 'r') as f:
    content = f.read()

search = """
pub(crate) mod persistent_engine;
pub(crate) mod storage;
"""

replace = """
pub mod persistent_engine;
pub mod storage;
"""

content = content.replace(search[1:], replace[1:])
with open('relvar-storage/src/lib.rs', 'w') as f:
    f.write(content)
