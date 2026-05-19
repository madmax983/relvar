import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: 🔒 Warden: [Fix image DoS with i128 span checks]\n\n{body}")
