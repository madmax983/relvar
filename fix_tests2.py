import os
import re

def fix_imports(file_path):
    with open(file_path, 'r') as f:
        content = f.read()

    # Change use relvar::tools::exporter; to use relvar::tools::{to_csv, to_json, to_ascii_table};
    content = re.sub(r'use relvar::tools::exporter;', r'use relvar::tools::{to_csv, to_json, to_ascii_table};', content)
    # Change exporter::to_csv to to_csv
    content = re.sub(r'exporter::to_csv', r'to_csv', content)

    # Fix warden_exploit_csv_global_dos.rs
    content = re.sub(r'use relvar::tools::\{self, ImporterError\};', r'use relvar::tools::{from_csv, ImporterError};', content)

    # Same for json dos
    content = re.sub(r'use relvar::tools::\{self, ImporterError\};', r'use relvar::tools::{from_json, ImporterError};', content)

    with open(file_path, 'w') as f:
        f.write(content)

for root, _, files in os.walk('relvar/tests'):
    for file in files:
        if file.endswith('.rs'):
            fix_imports(os.path.join(root, file))
