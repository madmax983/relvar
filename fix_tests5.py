import os
import re

def fix_imports(file_path):
    with open(file_path, 'r') as f:
        content = f.read()

    # import limits dos fix
    if "warden_import_limits.rs" in file_path:
        if "ImporterError" not in content[:200]:
            content = re.sub(r'use relvar::tools::\{from_json, from_csv, ImporterError\};', r'use relvar::tools::{from_json, from_csv, ImporterError};', content)
        # Check if the replace missed it
        if "use relvar::tools::{from_json, from_csv};" in content:
            content = re.sub(r'use relvar::tools::\{from_json, from_csv\};', r'use relvar::tools::{from_json, from_csv, ImporterError};', content)

    with open(file_path, 'w') as f:
        f.write(content)

for root, _, files in os.walk('relvar/tests'):
    for file in files:
        if file.endswith('.rs'):
            fix_imports(os.path.join(root, file))
