import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: ⚒️ Forge: Refactor God Function compute_pairwise_forces in Physics Engine\n\n{body}")
