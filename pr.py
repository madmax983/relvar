import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: 🛡️ Sentry: [test coverage improvement] Storage Heap Serialization Boundaries\n\n{body}")
