
import sys
import os

title = """⚡ Bolt: Avoid relation type heap allocation during DML updates"""
body = """💡 What:
Introduced `into_parts()` on `Relation` to return both the `RelationType` and its underlying `HashSet<Tuple>`, avoiding a `.clone()` call on the `RelationType`. Updated `compute_relation_after_update` in `relvar-core/src/database/dml.rs` to consume the relation via `into_parts()` instead of cloning its type and manually invoking `into_iter()`.

🎯 Why:
Cloning a `RelationType` internally allocates on the heap since it deeply clones the associated `TupleType` attributes vector. DML `UPDATE` statements are a critical path where creating unneeded heap allocations slows down performance over millions of evaluated tuples.

📊 Impact:
Eliminates one heap allocation per updated relation by safely decoupling the type heading from the tuple set body.

🔬 Measurement:
Run `cargo bench --bench database` to evaluate performance. Also `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all`, and `cargo test` pass successfully."""

print(f"Submitting PR with title: {title}\n\n{body}")
