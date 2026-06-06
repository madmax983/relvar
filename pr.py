import sys
import os

with open("pr_description.md") as f:
    body = f.read().strip()

print(f"Submitting PR with title: 🛡️ Sentry: [test coverage improvement]\n\n{body}")
