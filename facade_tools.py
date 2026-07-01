import os
import re

dir_path = 'relvar/src/tools'
files = [f for f in os.listdir(dir_path) if f.endswith('.rs') and f != 'mod.rs']

exports = []

for f in files:
    mod_name = f[:-3]
    content = open(os.path.join(dir_path, f)).read()

    matches = re.findall(r'^pub (struct|fn|trait|enum)\s+([A-Za-z0-9_]+)', content, re.MULTILINE)
    for m in matches:
        exports.append(f"pub use {mod_name}::{m[1]};")

print("\n".join(sorted(exports)))
