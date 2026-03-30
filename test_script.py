import re

with open("relvar-core/src/algebra/project.rs", "r") as f:
    content = f.read()

content = content.replace("use std::collections::BTreeMap;\n", "")

with open("relvar-core/src/algebra/project.rs", "w") as f:
    f.write(content)
