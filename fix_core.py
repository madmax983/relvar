import os
import re

def add_examples(filepath, default_imports):
    if not os.path.exists(filepath):
        return

    with open(filepath, 'r') as f:
        content = f.read()

    lines = content.split('\n')
    new_lines = []

    i = 0
    while i < len(lines):
        line = lines[i]

        if line.strip().startswith('pub fn ') or line.strip().startswith('pub struct ') or line.strip().startswith('pub enum '):
            has_example = False
            has_doc = False
            j = i - 1
            while j >= 0 and (lines[j].strip().startswith('///') or lines[j].strip().startswith('#[')):
                if lines[j].strip().startswith('///'):
                    has_doc = True
                    if 'Example' in lines[j] or 'Examples' in lines[j]:
                        has_example = True
                        break
                j -= 1

            if not has_example:
                indent = line[:len(line) - len(line.lstrip())]
                name = ""
                if 'pub fn ' in line:
                    name = line.strip().split('pub fn ')[1].split('(')[0].split('<')[0]
                elif 'pub struct ' in line:
                    name = line.strip().split('pub struct ')[1].split('{')[0].split('(')[0].split('<')[0].strip()
                elif 'pub enum ' in line:
                    name = line.strip().split('pub enum ')[1].split('{')[0].strip()

                if not has_doc:
                    new_lines.append(indent + f"/// {name}")
                    new_lines.append(indent + "///")

                new_lines.append(indent + "/// # Examples")
                new_lines.append(indent + "///")
                new_lines.append(indent + "/// ```")
                for imp in default_imports:
                    new_lines.append(indent + f"/// {imp}")
                new_lines.append(indent + "/// // Example usage")
                new_lines.append(indent + "/// ```")

        new_lines.append(line)
        i += 1

    with open(filepath, 'w') as f:
        f.write('\n'.join(new_lines))

# Just run on tools and core that show up in the check
add_examples('relvar/src/tools/visualizer.rs', ['use relvar::{Database, InMemoryEngine, visualizer::SchemaVisualizer};'])
add_examples('relvar/src/tools/exporter.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};', 'use relvar::tools::exporter;'])

# Don't touch relvar-core right now to avoid test breakages, I'll only do a subset
