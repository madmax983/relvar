import os
import re

def check_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    lines = content.split('\n')

    for i, line in enumerate(lines):
        if line.strip().startswith('pub fn ') or line.strip().startswith('pub struct ') or line.strip().startswith('pub enum '):
            if i > 0 and not lines[i-1].strip().startswith('///') and not lines[i-1].strip().startswith('#['):
                print(f"Undocumented at {filepath}:{i+1} -> {line.strip()}")
            elif i > 0 and lines[i-1].strip().startswith('///'):
                # Check for examples
                has_example = False
                j = i - 1
                while j >= 0 and lines[j].strip().startswith('///'):
                    if 'Examples' in lines[j]:
                        has_example = True
                    j -= 1
                if not has_example:
                    print(f"Missing example at {filepath}:{i+1} -> {line.strip()}")

for root, _, files in os.walk('relvar-core/src'):
    for file in files:
        if file.endswith('.rs'):
            check_file(os.path.join(root, file))

for root, _, files in os.walk('relvar/src'):
    for file in files:
        if file.endswith('.rs'):
            check_file(os.path.join(root, file))
