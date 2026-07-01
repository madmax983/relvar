import re

with open('relvar/src/lib.rs', 'r') as f:
    content = f.read()

# Remove the deprecated re-exports
content = re.sub(r'#\[deprecated\(note = "Use `relvar::tools::visualizer` instead"\)\]\npub use tools::visualizer;\n', '', content)
content = re.sub(r'/// Data import and export functionality\.\n#\[deprecated\(note = "Use `relvar::tools` instead"\)\]\npub mod data \{\n    pub use crate::tools::exporter;\n    pub use crate::tools::importer;\n\}\n', '', content)

with open('relvar/src/lib.rs', 'w') as f:
    f.write(content)
