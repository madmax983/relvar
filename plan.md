1. **Create Journal Entry:** Write an entry to `.jules/atlas.md` about the module visibility refactor using `run_in_bash_session` with `cat << 'EOF' >> .jules/atlas.md` containing the literal template format `YYYY-MM-DD`.
2. **Verify Journal Entry:** Run `cat .jules/atlas.md` to verify the journal entry was appended correctly.
3. **Create Script to Rewrite PR Script:** Create `rewrite_pr.py` using `run_in_bash_session` with `cat << 'EOF' > rewrite_pr.py` to generate the final `pr.py` file with the dynamic title and body from `pr_description.md`.
4. **Verify Rewrite PR Script:** Run `cat rewrite_pr.py` to verify the script contents.
5. **Rewrite PR Script:** Execute `python3 rewrite_pr.py`.
6. **Verify Modified PR Script:** Run `cat pr.py` to verify the dynamically injected title and body.
7. **Clean Up:** Delete `rewrite_pr.py` and `pr_description.md` using `rm` to maintain repository hygiene.
8. **Verify Refactoring - Format:** Run `cargo fmt --all` to format the workspace.
9. **Verify Refactoring - Clippy:** Run `cargo clippy --all-targets --all-features -- -D warnings` to verify strict linting rules.
10. **Verify Refactoring - Test:** Run `cargo test` to execute all tests.
11. **Verify Refactoring - Check:** Run `cargo check` to perform a fast build check.
12. **Verify Refactoring - Doc:** Run `cargo doc --open` to build documentation without missing doc warnings.
13. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
14. **Submit the Pull Request:** Execute `python3 pr.py` using `run_in_bash_session` to submit the PR.
