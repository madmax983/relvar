import sys
import os

with open("pr_description.md") as f:
    lines = f.readlines()
    if not lines:
        sys.exit(1)

    title = lines[0].strip()
    body = "".join(lines[1:]).strip()

print(f"Submitting PR with title: {title}\n\n{body}")
