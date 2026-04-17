import re

with open('relvar-storage/src/storage/heap.rs', 'r') as f:
    content = f.read()

content = content.replace("use relvar_core::tuple;", "#[allow(unused_imports)]\n        use relvar_core::tuple;")
content = content.replace("#[allow(unused_imports)]\n        #[allow(unused_imports)]\n        use relvar_core::tuple;", "#[allow(unused_imports)]\n        use relvar_core::tuple;")

with open('relvar-storage/src/storage/heap.rs', 'w') as f:
    f.write(content)
