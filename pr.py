import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: 🔒 Warden: Integer Overflow DoS in Convolution Filters\n\n{body}")
