import ast
import os
import re

def parse_rust_file(filepath):
    funcs = []
    with open(filepath, 'r') as f:
        content = f.read()

    # Strip multiline comments and string literals to avoid false braces
    content = re.sub(r'/\*.*?\*/', '', content, flags=re.DOTALL)
    # Basic string removal (imperfect but better than nothing)
    content = re.sub(r'".*?(?<!\\)"', '""', content)

    lines = content.split('\n')
    i = 0
    while i < len(lines):
        line = lines[i]

        # very basic test skipping
        if '#[test]' in line or '#[cfg(test)]' in line:
            # wait for next fn
            while i < len(lines) and ' fn ' not in lines[i] and ' mod ' not in lines[i]:
                i += 1

            if i < len(lines) and ' mod ' in lines[i]:
                # skip entire module block
                open_b = lines[i].count('{') - lines[i].count('}')
                started = '{' in lines[i]
                j = i + 1
                while j < len(lines) and (open_b > 0 or not started):
                    if '{' in lines[j]: open_b += 1; started = True
                    if '}' in lines[j]: open_b -= 1
                    j += 1
                i = j
                continue
            elif i < len(lines) and ' fn ' in lines[i]:
                open_b = lines[i].count('{') - lines[i].count('}')
                started = '{' in lines[i]
                j = i + 1
                while j < len(lines) and (open_b > 0 or not started):
                    if '{' in lines[j]: open_b += 1; started = True
                    if '}' in lines[j]: open_b -= 1
                    j += 1
                i = j
                continue

        match = re.search(r'^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(', line)
        if match:
            name = match.group(1)
            start_line = i
            open_b = line.count('{') - line.count('}')
            started = '{' in line
            j = i + 1
            while j < len(lines) and (open_b > 0 or not started):
                if '{' in lines[j]: open_b += 1; started = True
                if '}' in lines[j]: open_b -= 1
                j += 1

            length = j - start_line
            if length > 50:
                funcs.append((length, filepath, start_line + 1, name))

            i = j - 1
        i += 1
    return funcs

all_funcs = []
for root, dirs, files in os.walk('relvar-core/src'):
    for f in files:
        if f.endswith('.rs'):
            all_funcs.extend(parse_rust_file(os.path.join(root, f)))

for length, filepath, line, name in sorted(all_funcs, reverse=True)[:15]:
    print(f"{filepath}:{line} - {name} ({length} lines)")
