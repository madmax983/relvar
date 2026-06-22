import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: 🗺️ Atlas: Modularization of HeapFile Blob\n\n{body}")
