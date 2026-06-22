import sys
import os

with open("pr_description.md") as f:
    lines = f.readlines()
    if lines:
        title = lines[0].strip()
        body = "".join(lines[1:]).strip()
    else:
        title = "PR"
        body = ""

print(f"Submitting PR with title: {title}\n\n{body}")
