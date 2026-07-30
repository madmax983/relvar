Title: 🎻 Bard: [documentation update]

📖 Chapter: Added comprehensive executable doc-tests to the `Spreadsheet` module and properly marked internal serialization macro docs in `recursion.rs` as text.

🔦 Insight: The experimental `Spreadsheet` was poorly documented with a placeholder example, leaving users confused as to how relation types are applied to evaluate cell values recursively. The recursion guards were throwing invalid rust codeblock warnings that were noisy in `cargo doc`.

🧪 Example: Added an executable doctest to `Spreadsheet` which constructs relations, performs insertions, and asserts the correct evaluated fixpoint. Changed the `/// ```\n/// // Internally used by serde` block to `/// ```text` in `relvar-core/src/utils/recursion.rs`.
