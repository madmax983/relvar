import sys
import os

with open("pr_description.md") as f:
    body = f.read()

lines = body.split("\n")
title = lines[0] if lines else ""
print(f"Submitting PR with title: {title}\n\n{body}")
