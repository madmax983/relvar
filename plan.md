1. **Analyze God Function**: The `evaluate` method in `relvar/src/experimental/spreadsheet.rs` is a large function that combines finding unresolved formulas, joining values, evaluating the mathematical formulas, and unioning back into the result set. This perfectly fits the "Three-Phase Operator" pattern identified in the journals.
2. **Refactor using Helper Functions**:
   - Extract `find_unresolved_formulas(&self, current_values: &Relation) -> Result<Relation, DatabaseError>`
   - Extract `resolve_arguments(unresolved_formulas: &Relation, current_values: &Relation) -> Result<Relation, DatabaseError>`
   - Extract `evaluate_formulas(fully_resolved_args: &Relation) -> Result<Relation, DatabaseError>`
3. **Write and apply changes**: Modify `spreadsheet.rs` with `replace_with_git_merge_diff`.
4. **Update Journal**: Add a learning entry to `.jules/forge.md` about refactoring the `evaluate` method in `spreadsheet.rs`.
5. **Format & Test**: Run `cargo fmt`, `cargo clippy`, and `cargo test`.
6. **Pre-commit Check**: Follow the `pre_commit_instructions`.
7. **Submit PR**: Submit the pull request using `pr.py` via `run_in_bash_session` to fulfill the task requirements.
