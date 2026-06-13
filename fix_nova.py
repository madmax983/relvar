with open(".jules/nova.md") as f:
    lines = f.readlines()

new_lines = []
skip = False
for i, line in enumerate(lines):
    if "## Relational Garbage Collector" in line:
        # Check if we already added it
        if "## Relational Garbage Collector" in "".join(new_lines):
            skip = True

    if skip and line.startswith("## ") and "## Relational Garbage Collector" not in line:
        skip = False

    if not skip:
        new_lines.append(line)

with open(".jules/nova.md", "w") as f:
    f.writelines(new_lines)
