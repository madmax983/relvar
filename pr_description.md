# ⚒️ Forge: Fix for-kv-map clippy warning in timeseries.rs

🚮 Smell: Iterating over keys and values using `for (attr_name, _) in original_heading.attributes().iter()` when only the key is used, which triggers a `for-kv-map` Clippy warning.
✨ Solution: Replaced the iterator with `.keys()` since the value is unused.
🧼 Benefit: Resolves the Clippy warning and aligns the code with idiomatic Rust.
🛡️ Verification: Tests passed. No logic changed.
