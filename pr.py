import sys
import os

with open("pr_description.md") as f:
    lines = f.read().splitlines()

if len(lines) > 0 and lines[0].startswith("🗺️ Atlas:"):
    title = lines[0]
    body = "\n".join(lines[1:]).strip()
else:
    title = "🗺️ Atlas: Architectural change"
    body = "\n".join(lines)

print(f"Submitting PR with title: {title}\n\n{body}")
