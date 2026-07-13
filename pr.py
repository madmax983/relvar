import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: 🗺️ Atlas: Enforce strict boundaries with PageId NewType\n\n{body}")
