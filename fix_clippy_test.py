import re

with open('relvar-storage/src/storage/heap.rs', 'r') as f:
    content = f.read()

content = content.replace("use relvar_core::tuple;\n        use relvar_core::types::{ScalarType, TupleType};\n", "#[allow(unused_imports)]\n        use relvar_core::tuple;\n        #[allow(unused_imports)]\n        use relvar_core::types::{ScalarType, TupleType};\n")
content = content.replace("use relvar_core::tuple;\n", "#[allow(unused_imports)]\n        use relvar_core::tuple;\n")

# Revert the doc test modification since I'm just replacing `use relvar_core::tuple;` globally...
content = content.replace("/// #[allow(unused_imports)]\n        use relvar_core::tuple;\n", "/// use relvar_core::tuple;\n")

with open('relvar-storage/src/storage/heap.rs', 'w') as f:
    f.write(content)
