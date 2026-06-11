import sys
import os

with open("pr_description.md") as f:
    lines = f.readlines()
    title = lines[0].strip()
    body = "".join(lines[1:])

print(f"Submitting PR with title: {title}\n\n{body}")
