import sys
import os

print(f"""Submitting PR with title: ⚡ Bolt: Use into_parts to avoid relation type cloning during DML updates

💡 What: Added `into_parts()` to `Relation` and utilized it in DML updates.
🎯 Why: DML update iteration was unnecessarily cloning the `RelationType` to capture it, creating heap allocations (specifically Arc clones within relation_type.clone()) on each update operation.
📊 Impact: Removes `RelationType` cloning during `UPDATE`s by deconstructing the existing `Relation` via `into_parts`, optimizing memory usage inside hot DML loops without lifetime headaches.
🔬 Measurement: Verify tests run fine using `cargo test` and `cargo bench --bench database`.""")
