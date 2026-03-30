1. **Refactor `ScalarValue::eq` and `Hash` in `relvar-core/src/values/scalar.rs`**
   - The `eq` method in `PartialEq` implementation is a nested `match` block.
   - We have viewed the full `eq` and `hash` implementations. The `UserDefined` loop logic in `eq` spans lines 267-291, and the `UserDefined` loop logic in `hash` spans lines 336-353.
   - We will extract `eq_user_defined_values` into `impl ScalarValue` to match the `cmp_user_defined_values` pattern that was confirmed to exist (lines 464-502).
   - We will extract `eq_floats` into `impl ScalarValue` to match the `cmp_floats` pattern that was confirmed to exist (lines 415-450).
   - We will refactor `eq` to use an early return: `if std::mem::discriminant(self) != std::mem::discriminant(other) { return false; }`. This flattens the match arms, replacing the nested `match (self, other)` logic.
   - For `Hash`, we will extract a `hash_user_defined_values` helper method inside `impl ScalarValue` and call it from the `Hash` implementation to flatten the function.
   - I will use `replace_with_git_merge_diff` to make these changes.

2. **Run tests & clippy**
   - Run `cargo clippy --all-targets --all-features -- -D warnings` using `run_in_bash_session`.
   - Run `cargo test` using `run_in_bash_session` to ensure these refactors do not break behavior.
   - Run `cargo fmt --all` using `run_in_bash_session`.

3. **Complete pre-commit steps**
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done using `pre_commit_instructions`.

4. **Submit PR**
   - Use `submit` with branch `forge/refactor-scalar-eq`
   - Title: `⚒️ Forge: refactor ScalarValue eq and hash methods`
   - Describe Smell, Solution, Benefit, and Verification according to Forge's process.
