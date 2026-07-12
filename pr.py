import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: 🔒 Warden: Prevent arithmetic overflow DoS in image convolution\n\n{body}")
