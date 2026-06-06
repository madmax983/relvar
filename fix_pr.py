with open("pr.py", 'r') as f:
    content = f.read()

content = """
import sys
import os

with open("pr_description.md") as f:
    lines = f.readlines()

title = lines[0].strip()
body = "".join(lines[1:]).strip()

print(f"Submitting PR with title: {title}\\n\\n{body}")
"""

with open("pr.py", "w") as f:
    f.write(content)
