import os
import re

def find_functions():
    func_pattern = re.compile(r'^\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(')

    results = []

    for root, dirs, files in os.walk('.'):
        for file in files:
            if not file.endswith('.rs'):
                continue

            filepath = os.path.join(root, file)
            with open(filepath, 'r') as f:
                lines = f.readlines()

            i = 0
            while i < len(lines):
                line = lines[i]

                # Skip #[cfg(test)] modules entirely
                if '#[cfg(test)]' in line.replace(" ", ""):
                    # Find the end of the test module
                    open_braces = 0
                    started = False
                    while i < len(lines):
                        if '{' in lines[i]:
                            open_braces += lines[i].count('{')
                            started = True
                        if '}' in lines[i]:
                            open_braces -= lines[i].count('}')

                        if started and open_braces == 0:
                            break
                        i += 1
                    continue

                match = func_pattern.search(line)
                if match:
                    func_name = match.group(1)

                    # Track function length
                    open_braces = line.count('{') - line.count('}')
                    started = '{' in line

                    j = i + 1
                    while j < len(lines) and (open_braces > 0 or not started):
                        if '{' in lines[j]:
                            open_braces += lines[j].count('{')
                            started = True
                        if '}' in lines[j]:
                            open_braces -= lines[j].count('}')

                        if started and open_braces == 0:
                            break
                        j += 1

                    length = j - i + 1
                    if length > 50:
                        results.append((length, filepath, i+1, func_name))

                i += 1

    return sorted(results, reverse=True)

for length, filepath, line, name in find_functions():
    print(f"{filepath}:{line} - {name} ({length} lines)")
