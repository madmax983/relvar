import sys

with open("relvar-core/src/algebra/mod.rs", 'r') as f:
    content = f.read()

import re
content = re.sub(r'//! - \[`delta`\]\(algebra::delta\): Compute changes between relations.\n', '', content)
content = re.sub(r'//! - \[`Delta`\]\(algebra::delta::Delta\): Represents changes \(inserts/deletes\) between relations\.\n', '', content)

with open("relvar-core/src/algebra/mod.rs", 'w') as f:
    f.write(content)
