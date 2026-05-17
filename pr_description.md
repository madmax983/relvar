📖 Chapter: The `Spreadsheet` Experimental Engine
🔦 Insight: Executable doc-tests have been systematically added to `Spreadsheet`'s `new` and `evaluate` methods. Previously, these functions contained empty placeholder examples which made it confusing to understand how to initialize the relations representing values and formulas or how the fixpoint evaluation executed the arithmetic formulas purely using relational algebra.
🧪 Example: Added 2 comprehensive, executable doctests demonstrating how to populate raw values, define "ADD" formulas linking cells, and successfully evaluate the spreadsheet.
🖼️ Preview: Evaluated the addition formula natively during the doc-tests via `cargo test --doc`.
