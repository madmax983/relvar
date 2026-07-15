import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: 🔒 Warden: Prevent Denial of Service in Image Processing via Memory Allocation Exhaustion\n\n{body}")
