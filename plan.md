1. **Document `Spreadsheet` in `relvar/src/experimental/spreadsheet.rs`**
   - Use `replace_with_git_merge_diff` to add module level `//!` docs at the top of the file, explaining the spreadsheet engine concept, and also to add `## Examples` doc-tests on the `Spreadsheet` struct and its methods `new` and `evaluate` replacing the placeholder comments with actual working examples that show creating relations and evaluating a formula.

2. **Verify changes**
   - Execute `run_in_bash_session` with `cargo test`
   - Execute `run_in_bash_session` with `cargo fmt --all`
   - Execute `run_in_bash_session` with `cargo clippy --all-targets --all-features -- -D warnings`
   - Execute `run_in_bash_session` with `cargo check`
   - Execute `run_in_bash_session` with `cargo doc --open`

3. **Create PR description**
   - Create `pr_description.md` using `run_in_bash_session` to run `cat << 'EOF' > pr_description.md` with the PR title and description format.

4. **Pre-commit checks**
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.

5. **Submit PR**
   - Run `pr.py` to create a PR.
