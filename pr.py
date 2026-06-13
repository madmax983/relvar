import sys
import os

with open("pr_description.md") as f:
    lines = f.read().strip().split('\n')
    title = lines[0]
    body = '\n'.join(lines[1:])

print(f"Submitting PR with title: {title}\n\n{body}")
