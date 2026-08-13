with open('relvar/src/lib.rs', 'r') as f:
    content = f.read()

content = content.replace("pub(crate) mod experimental;", "pub mod experimental;")
content = content.replace("pub(crate) mod tools;", "pub mod tools;")

with open('relvar/src/lib.rs', 'w') as f:
    f.write(content)

with open('relvar-storage/src/lib.rs', 'r') as f:
    content = f.read()

content = content.replace("pub(crate) mod persistent_engine;", "pub mod persistent_engine;")
content = content.replace("pub(crate) mod storage;", "pub mod storage;")

with open('relvar-storage/src/lib.rs', 'w') as f:
    f.write(content)
