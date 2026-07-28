Title: 🎻 Bard: [Document Spreadsheet Engine]

📖 Chapter: Documented the `Spreadsheet` module within `experimental`.

🔦 Insight: The `Spreadsheet` engine was missing an executable doc-test for its constructor and `evaluate` method. Added these tests to ensure users understand how to initialize a spreadsheet from relational values and formulas, and correctly invoke the evaluation loop. This clearly illustrates the "Relational Spreadsheet" concept.

🧪 Example: Added 2 executable doctests to `relvar/src/experimental/spreadsheet.rs` for `Spreadsheet::new` and `Spreadsheet::evaluate`.

🖼️ Preview: Evaluated spreadsheet rows correctly resolving `val`.
