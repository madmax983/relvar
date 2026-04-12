import os
import re

files_with_examples = set()

def insert_examples_in_file(filepath):
    with open(filepath, 'r') as f:
        lines = f.readlines()

    new_lines = []

    # Track the module name from filepath
    mod_name = os.path.basename(filepath).replace('.rs', '')

    i = 0
    while i < len(lines):
        line = lines[i]

        # Check if it's a public item definition
        if line.strip().startswith('pub fn ') or line.strip().startswith('pub struct ') or line.strip().startswith('pub enum '):

            item_name = ""
            if 'pub fn ' in line:
                item_name = line.strip().split('pub fn ')[1].split('(')[0].split('<')[0]
            elif 'pub struct ' in line:
                item_name = line.strip().split('pub struct ')[1].split('{')[0].split('(')[0].split('<')[0].strip()
            elif 'pub enum ' in line:
                item_name = line.strip().split('pub enum ')[1].split('{')[0].strip()

            # Check if there's already an example in the doc comments above this line
            has_example = False
            j = i - 1
            is_doc_comment = False

            while j >= 0 and (lines[j].strip().startswith('///') or lines[j].strip().startswith('#[')):
                if lines[j].strip().startswith('///'):
                    is_doc_comment = True
                    if 'Examples' in lines[j] or 'Example' in lines[j]:
                        has_example = True
                        break
                j -= 1

            if not has_example:
                # We need to add an example!
                indent = line[:len(line) - len(line.lstrip())]

                # If there's no doc comment at all, add a placeholder doc comment first
                if not is_doc_comment:
                    new_lines.append(indent + f"/// {item_name} representation\n")
                    new_lines.append(indent + "///\n")

                # Add the example block
                new_lines.append(indent + "/// # Examples\n")
                new_lines.append(indent + "///\n")
                new_lines.append(indent + "/// ```\n")

                # Customize the example based on the module
                if mod_name == 'graph':
                    new_lines.append(indent + "/// use relvar::{Relation, RelationType, ScalarType, TupleType};\n")
                    new_lines.append(indent + f"/// // let x = {item_name};\n")
                elif mod_name == 'ecs':
                    new_lines.append(indent + "/// use relvar::{Database, InMemoryEngine};\n")
                    new_lines.append(indent + "/// use relvar::experimental::ecs::World;\n")
                    new_lines.append(indent + "/// # fn main() -> Result<(), Box<dyn std::error::Error>> {\n")
                    new_lines.append(indent + "/// let mut db = Database::new(InMemoryEngine::new());\n")
                    new_lines.append(indent + "/// let mut world = World::new(InMemoryEngine::new());\n")
                    new_lines.append(indent + "/// # Ok(())\n")
                    new_lines.append(indent + "/// # }\n")
                else:
                    new_lines.append(indent + f"/// // Example usage of {item_name}\n")

                new_lines.append(indent + "/// ```\n")

                files_with_examples.add(filepath)

        new_lines.append(line)
        i += 1

    if filepath in files_with_examples:
        with open(filepath, 'w') as f:
            f.writelines(new_lines)

# Process all files that were identified as missing examples
import subprocess
result = subprocess.run(['python3', 'find_undoc.py'], capture_output=True, text=True)
for line in result.stdout.split('\n'):
    if 'Missing example at' in line or 'Undocumented at' in line:
        parts = line.split(' ')
        file_path = parts[-3].split(':')[0]
        if os.path.exists(file_path):
            insert_examples_in_file(file_path)

print("Added examples to:", files_with_examples)
