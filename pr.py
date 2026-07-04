import sys
import os

with open("pr_description.md") as f:
    title = f.readline().strip()
    body = f.read()

print(f"Submitting PR with title: {title}\n\n{body}")
