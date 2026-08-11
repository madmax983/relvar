with open("relvar/src/lib.rs", "r") as f:
    content = f.read()

content = content.replace("pub(crate) mod experimental;", "pub mod experimental;")
content = content.replace("pub(crate) mod tools;", "pub mod tools;")

with open("relvar/src/lib.rs", "w") as f:
    f.write(content)
