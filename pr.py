import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: ⚒️ Forge: Refactor compute_pairwise_forces\n\n{body}")
