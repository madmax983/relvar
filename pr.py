import sys
import os
import requests

with open("pr_description.md") as f:
    body = f.read()

title = body.split("\n")[0].strip()

print(f"Submitting PR with title: {title}\n\n{body}")
