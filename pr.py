import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: 🔒 Warden: Fix RUSTSEC-2026-0190\n\n{body}")
