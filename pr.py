import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: 🌟 Nova: Relational Mark-and-Sweep Garbage Collector\n\n{body}")
