1. **Document `Spreadsheet` struct in `relvar/src/experimental/spreadsheet.rs`**
   - Replace the missing example in `Spreadsheet` with an executable doctest showing how to use the spreadsheet engine.
   - Update `Spreadsheet::new` to have an executable doctest.
   - Update `Spreadsheet::evaluate` to have an executable doctest.
2. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
   - Run `cargo fmt --all`.
   - Run `cargo clippy --all-targets --all-features -- -D warnings`.
   - Run `cargo test`.
   - Run `cargo doc --open`.
   - Add journal entry to `.jules/bard.md`.
3. **Submit the PR**
   - Use `pr.py` script via `run_in_bash_session` to create PR with the title "🎻 Bard: [documentation update]".
