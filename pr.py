import sys
import os

with open("pr_description.md") as f:
    body = f.read()

print(f"Submitting PR with title: ⚒️ Forge: Refactor `recover` function in `relvar-storage/src/wal/recovery.rs`\n\n{body}")
