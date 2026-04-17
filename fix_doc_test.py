import re

with open('relvar-storage/src/storage/heap.rs', 'r') as f:
    content = f.read()

# Fix the mess I just made in the doc test
content = content.replace("/// #[allow(unused_imports)]\n        use relvar_core::tuple;\n///", "/// use relvar_core::tuple;\n///")
content = content.replace("#[allow(unused_imports)]\n        use relvar_core::tuple;", "use relvar_core::tuple;")

# Now replace the actual issue at line 112 (which I probably broke). Ah, wait, line 112 is INSIDE the doc comment.
content = content.replace("use relvar_core::tuple;\n///\n/// // Create heap file", "use relvar_core::tuple;\n///\n/// // Create heap file")

with open('relvar-storage/src/storage/heap.rs', 'w') as f:
    f.write(content)
