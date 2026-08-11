with open("relvar-storage/src/lib.rs", "r") as f:
    content = f.read()

content = content.replace("pub(crate) mod storage;", "pub mod storage;")

with open("relvar-storage/src/lib.rs", "w") as f:
    f.write(content)
