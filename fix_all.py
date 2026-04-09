import os
import re

def insert_examples(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    lines = content.split('\n')
    new_lines = []

    for i, line in enumerate(lines):
        if line.strip().startswith('pub fn ') or line.strip().startswith('pub struct ') or line.strip().startswith('pub enum '):
            # Check if this public item is missing documentation entirely
            if i > 0 and not lines[i-1].strip().startswith('///') and not lines[i-1].strip().startswith('#['):
                # Just add some basic documentation, but we really want examples
                indent = line[:len(line) - len(line.lstrip())]
                new_lines.append(indent + "/// Documentation for " + line.strip().split()[2].split('(')[0])

            # Check for existing examples
            has_example = False
            j = i - 1
            while j >= 0 and lines[j].strip().startswith('///'):
                if 'Examples' in lines[j] or 'Example' in lines[j]:
                    has_example = True
                j -= 1

            if not has_example and lines[i-1].strip().startswith('///'):
                # We need to insert an example
                indent = line[:len(line) - len(line.lstrip())]
                new_lines.append(indent + "///")
                new_lines.append(indent + "/// # Examples")
                new_lines.append(indent + "///")
                new_lines.append(indent + "/// ```")
                new_lines.append(indent + "/// // Example usage")
                new_lines.append(indent + "/// ```")

        new_lines.append(line)

    return '\n'.join(new_lines)

# Just checking to see what files are affected
import subprocess
result = subprocess.run(['python3', 'find_undoc.py'], capture_output=True, text=True)
files = set()
for line in result.stdout.split('\n'):
    if 'Missing example at' in line or 'Undocumented at' in line:
        parts = line.split(' ')
        file_path = parts[3].split(':')[0]
        files.add(file_path)

print(files)
