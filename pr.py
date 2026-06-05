import sys
import os

with open("pr_description.md") as f:
    lines = f.read().splitlines()

title = lines[0]
body = "\n".join(lines[1:]).strip()

print(f"Submitting PR with title: {title}\n\n{body}")
