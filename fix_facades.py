import os
import re

def update_mod_file(dir_path):
    mod_path = os.path.join(dir_path, 'mod.rs')
    with open(mod_path, 'r') as f:
        content = f.read()

    # Change all `pub mod <name>;` to `pub(crate) mod <name>;`
    content = re.sub(r'^pub mod ([A-Za-z0-9_]+);', r'pub(crate) mod \1;', content, flags=re.MULTILINE)

    # Generate `pub use` statements
    files = [f for f in os.listdir(dir_path) if f.endswith('.rs') and f != 'mod.rs']
    exports = []

    for f in files:
        mod_name = f[:-3]
        f_path = os.path.join(dir_path, f)
        with open(f_path, 'r') as mf:
            m_content = mf.read()

        matches = re.findall(r'^pub (struct|fn|trait|enum)\s+([A-Za-z0-9_]+)', m_content, re.MULTILINE)
        for m in matches:
            exports.append(f"pub use {mod_name}::{m[1]};")

    exports.sort()

    if exports:
        content += "\n// --- Facade Re-exports ---\n"
        content += "\n".join(exports)
        content += "\n"

    with open(mod_path, 'w') as f:
        f.write(content)

update_mod_file('relvar/src/experimental')
update_mod_file('relvar/src/tools')
