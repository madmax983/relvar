import os
import re

def fix_imports(file_path):
    with open(file_path, 'r') as f:
        content = f.read()

    # Change relvar::tools::importer::X to relvar::tools::X
    content = re.sub(r'relvar::tools::importer::', r'relvar::tools::', content)

    # Change relvar::tools::visualizer::X to relvar::tools::X
    content = re.sub(r'relvar::tools::visualizer::', r'relvar::tools::', content)

    # Change relvar::tools::exporter::X to relvar::tools::X
    content = re.sub(r'relvar::tools::exporter::', r'relvar::tools::', content)

    # Same for experimental
    content = re.sub(r'relvar::experimental::[a-zA-Z0-9_]+::', r'relvar::experimental::', content)

    # For `use relvar::tools::importer;` we need to fix it to just `use relvar::tools::from_json;` etc. if it's used that way.
    # It's easier to just replace `importer::` with `tools::` where it occurs.

    with open(file_path, 'w') as f:
        f.write(content)

for root, _, files in os.walk('relvar/tests'):
    for file in files:
        if file.endswith('.rs'):
            fix_imports(os.path.join(root, file))

# And in benches
for root, _, files in os.walk('benches'):
    for file in files:
        if file.endswith('.rs'):
            fix_imports(os.path.join(root, file))

# Also check integration files if any in relvar
