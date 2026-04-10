import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: ⚡ Bolt: Avoid cloning heading per tuple in ungroup\n\n{body}")
