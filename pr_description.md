Title: 🎻 Bard: [documentation update]

📖 Chapter: The `Spreadsheet` module (experimental).
🔦 Insight: The experimental `Spreadsheet` engine was missing executable doc-tests, making it confusing to understand how values and formulas relations interact to solve the fixpoint spreadsheet evaluation using relational algebra. We have added proper `/// # Examples` that compile and execute as part of the test suite.
🧪 Example: Added 3 executable doctests for `Spreadsheet`, `Spreadsheet::new`, and `Spreadsheet::evaluate`.
🖼️ Preview: Documentation renders cleanly when running `cargo doc --open`.
