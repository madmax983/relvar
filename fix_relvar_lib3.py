with open("relvar/src/lib.rs", "r") as f:
    content = f.read()

content = content.replace("use relvar::Aggregation;", "use relvar_core::Aggregation;")

with open("relvar/src/lib.rs", "w") as f:
    f.write(content)
