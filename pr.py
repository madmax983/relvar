import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: 🔒 Warden: [Fix recursion DoS in ConstraintExpression deserialization]\n\n{body}")
