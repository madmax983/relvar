import re

with open('relvar/src/tools/visualizer.rs', 'r') as f:
    content = f.read()

new_content = re.sub(r'use relvar::\{Database, InMemoryEngine, SchemaVisualizer\};', r'use relvar::{Database, InMemoryEngine, tools::SchemaVisualizer};', content)

with open('relvar/src/tools/visualizer.rs', 'w') as f:
    f.write(new_content)
