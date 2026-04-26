with open('relvar-core/src/types/scalar.rs', 'r') as f:
    content = f.read()

content = content.replace("""        match self {
            ScalarType::Int => {}
            ScalarType::Float => {}
            ScalarType::String => {}
            ScalarType::Bool => {}
            ScalarType::Bytes => {}
            ScalarType::Relation(rel_type) => {
                // Hash the relation type's heading
                rel_type.heading().hash(state);
            }
            ScalarType::UserDefined {
                name,
                representation,
            } => {
                name.hash(state);
                representation.hash(state);
            }
        }""", """        match self {
            ScalarType::Int | ScalarType::Float | ScalarType::String | ScalarType::Bool | ScalarType::Bytes => {}
            ScalarType::Relation(rel_type) => rel_type.heading().hash(state),
            ScalarType::UserDefined { name, representation } => {
                name.hash(state);
                representation.hash(state);
            }
        }""")

with open('relvar-core/src/types/scalar.rs', 'w') as f:
    f.write(content)
