import re

with open("relvar-core/src/algebra/union.rs", "r") as f:
    content = f.read()

content = content.replace(
"""        for tuple in other.tuples() {
            self.insert(tuple.clone()).unwrap();
        }""",
"""        self.body.reserve(other.cardinality());
        self.body.extend(other.tuples().cloned());"""
)

with open("relvar-core/src/algebra/union.rs", "w") as f:
    f.write(content)
