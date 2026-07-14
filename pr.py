import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: ⚡ Bolt: Replace .collect() with BTreeMap::from_iter for fast initializations in rename\n\n{body}")
