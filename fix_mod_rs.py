with open('relvar-storage/src/storage/heap/tests/mod.rs', 'r') as f:
    lines = f.readlines()

new_lines = []
seen = set()
for line in lines:
    if line.strip() in seen and line.strip().startswith("mod "):
        continue
    if line.strip().startswith("mod "):
        seen.add(line.strip())
    new_lines.append(line)

with open('relvar-storage/src/storage/heap/tests/mod.rs', 'w') as f:
    f.writelines(new_lines)
