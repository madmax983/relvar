import sys

with open("relvar/src/lib.rs", "r") as f:
    content = f.read()

content = content.replace("use relvar::tools::{importer, exporter};", "use relvar::data::{importer, exporter};")

with open("relvar/src/lib.rs", "w") as f:
    f.write(content)
