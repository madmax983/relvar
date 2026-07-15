import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: ⚡ Bolt: Eliminate intermediate Vec allocation in project\n\n{body}")
