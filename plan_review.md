1. **Update `Spreadsheet` documentation**:
   - Add an executable `/// # Examples` block to `pub struct Spreadsheet` to demonstrate how the struct is used.
2. **Update `Spreadsheet::new` documentation**:
   - Replace the placeholder example with a working, executable doc-test for `Spreadsheet::new`.
3. **Update `Spreadsheet::evaluate` documentation**:
   - Replace the placeholder example with a working, executable doc-test for `Spreadsheet::evaluate`.
4. **Run Verification checks**:
   - Run `cargo test -p relvar --doc` to verify the new tests pass.
   - Run `cargo doc --open` to verify visual rendering.
   - Run `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --all`.
5. **Update `.jules/bard.md`**:
   - Add a journal entry detailing the clarification of the Spreadsheet module documentation.
6. **Pre-commit Steps**:
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
7. **Submit PR**:
   - Submit the PR with Bard's requested format.
