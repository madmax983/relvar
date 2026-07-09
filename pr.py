import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: ⚒️ Forge: extract duplicated closure logic in physics engine\n\n{body}")
