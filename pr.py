import sys
import os

body = """🚰 Smell: Deep folder hierarchies (`relvar-core/src/query/mod.rs`, `relvar-storage/src/storage/heap/mod.rs`, etc.) with 1-2 files per folder added unnecessary abstractions and made the codebase harder to navigate.
✨ Solution: Flattened deep folders by moving `mod.rs` contents directly into `<module>.rs` (e.g., `relvar-core/src/query.rs`).
🧼 Benefit: Much flatter and more intuitive project structure without unnecessary boilerplate nesting, abiding by the KISS principle.
🛡️ Verification: Ran `cargo test`, `cargo clippy`, and `cargo fmt`. All verify successfully and compilation continues to succeed. Documented in `.jules/razor.md`."""

print(f"Submitting PR with title: 🪒 Razor: Flatten module directories to eliminate bloat\n\n{body}")

with open("pr_description.md", "w") as f:
    f.write(body)
