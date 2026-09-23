with open("relvar/src/lib.rs", "r") as f:
    text = f.read()

text = text.replace("pub(crate) mod experimental;", "pub mod experimental;")
text = text.replace("pub(crate) mod tools;", "pub mod tools;")

with open("relvar/src/lib.rs", "w") as f:
    f.write(text)
