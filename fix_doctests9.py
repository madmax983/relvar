import sys

with open("relvar-core/src/algebra/mod.rs", 'r') as f:
    content = f.read()

import re
content = re.sub(r'pub use summarize::\{Aggregation, AggregationFn\};\n', '', content)

with open("relvar-core/src/algebra/mod.rs", 'w') as f:
    f.write(content)

with open("relvar-core/src/lib.rs", 'r') as f:
    content = f.read()

content = content.replace("pub use constraints::{", "pub use algebra::{Aggregation, AggregationFn};\n\npub use constraints::{")

with open("relvar-core/src/lib.rs", 'w') as f:
    f.write(content)
