import os
import re

def fix_imports(file_path):
    with open(file_path, 'r') as f:
        content = f.read()

    # json dos fix
    if "warden_json_import.rs" in file_path:
        content = re.sub(r'use relvar::tools::\{from_csv, ImporterError\};', r'use relvar::tools::{from_json, ImporterError};', content)

    # import limits
    if "warden_import_limits.rs" in file_path:
        content = re.sub(r'use relvar::tools::importer;', r'use relvar::tools::{from_json, from_csv, ImporterError};', content)
        content = re.sub(r'importer::from_json', r'from_json', content)
        content = re.sub(r'importer::from_csv', r'from_csv', content)
        content = re.sub(r'importer::ImporterError', r'ImporterError', content)

    # image dos fix
    if "warden_exploit_image_dos.rs" in file_path:
        content = re.sub(r'use relvar::experimental::image;', r'use relvar::experimental::{load as image_load};', content)
        content = re.sub(r'image::load', r'image_load', content)

    with open(file_path, 'w') as f:
        f.write(content)

for root, _, files in os.walk('relvar/tests'):
    for file in files:
        if file.endswith('.rs'):
            fix_imports(os.path.join(root, file))
