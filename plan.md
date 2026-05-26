1. **Add executable doc-tests to `Spreadsheet` module and struct**
   - Use `replace_with_git_merge_diff` to add module-level docs `//!` to `relvar/src/experimental/spreadsheet.rs`.
   - Add an executable `/// # Examples` block for `Spreadsheet`.
   - Update `Spreadsheet::new` and `Spreadsheet::evaluate` to have real executable doc-tests instead of placeholders.
2. **Verify documentation rendering and execution**
   - Run `cargo test --doc` to ensure all doctests compile and run correctly.
   - Run `cargo doc --no-deps --open` to check visual rendering (if applicable, or just build docs).
3. **Run standard checks**
   - Run `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo fmt --all`, and `cargo check`.
4. **Append learning to Bard's journal**
   - Append the learning regarding the spreadsheet module documentation to `.jules/bard.md`.
5. **Verify journal modification**
   - Execute `cat .jules/bard.md` to ensure the journal was updated correctly.
6. **Create PR description**
   - Use `write_file` to create `pr_description.md` with the required PR format (Chapter, Insight, Example, Preview).
7. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
   - Call `pre_commit_instructions` and follow the provided steps.
8. **Submit the PR**
   - Run `python3 pr.py` to submit the PR.
