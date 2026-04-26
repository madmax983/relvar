import sys
with open('relvar-core/src/types/scalar.rs', 'r') as f:
    content = f.read()

# Modify the formatting of panic to potentially appease Tarpaulin's span tracing
content = content.replace("""        if representation.depth() + 1 > crate::types::MAX_TYPE_DEPTH {
            panic!(
                "Type nesting too deep: {} (limit: {})",
                representation.depth() + 1,
                crate::types::MAX_TYPE_DEPTH
            );
        }""", """        if representation.depth() + 1 > crate::types::MAX_TYPE_DEPTH {
            panic!("Type nesting too deep: {} (limit: {})", representation.depth() + 1, crate::types::MAX_TYPE_DEPTH);
        }""")

with open('relvar-core/src/types/scalar.rs', 'w') as f:
    f.write(content)
