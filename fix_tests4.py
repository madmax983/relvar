import os
import re

def fix_imports(file_path):
    with open(file_path, 'r') as f:
        content = f.read()

    # image dos fix
    if "warden_exploit_image_dos.rs" in file_path:
        content = re.sub(r'use relvar::experimental::\{load as image_load\};', r'use relvar::experimental::{load as image_load, save as image_save};', content)
        content = re.sub(r'image::save', r'image_save', content)
        # Type inference fix
        content = re.sub(r'assert!\(data.is_empty\(\)\);', r'assert!(data.is_empty()); let _: Vec<u8> = data;', content)

    with open(file_path, 'w') as f:
        f.write(content)

for root, _, files in os.walk('relvar/tests'):
    for file in files:
        if file.endswith('.rs'):
            fix_imports(os.path.join(root, file))
