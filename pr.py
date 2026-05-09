import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: ⚡ Bolt: Avoid Vec allocation in query AST builder methods\n\n{body}")
