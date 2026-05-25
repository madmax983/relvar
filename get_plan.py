plan = """1. **Implement `IntoIterator` optimization for `project`**
   - Update `relvar-core/src/query/mod.rs` to change `project`'s signature from `pub fn project<S: Into<String>>(self, attributes: Vec<S>) -> Self` to `pub fn project<I, S>(self, attributes: I) -> Self where I: IntoIterator<Item = S>, S: Into<String>`.

2. **Implement `IntoIterator` optimization for `rename`**
   - Update `relvar-core/src/query/mod.rs` to change `rename`'s signature from `pub fn rename<S1: Into<String>, S2: Into<String>>(self, mappings: Vec<(S1, S2)>) -> Self` to `pub fn rename<I, S1, S2>(self, mappings: I) -> Self where I: IntoIterator<Item = (S1, S2)>, S1: Into<String>, S2: Into<String>`.

3. **Implement `IntoIterator` optimization for `summarize`**
   - Update `relvar-core/src/query/mod.rs` to change `summarize`'s signature to use `IntoIterator` for `group_by` and `aggregations` arguments: `pub fn summarize<I, S, A>(self, group_by: I, aggregations: A) -> Self where I: IntoIterator<Item = S>, S: Into<String>, A: IntoIterator<Item = Aggregation>`.

4. **Verify changes compile and pass tests**
   - Run `cargo check --all-targets --all-features`.
   - Run `cargo clippy --all-targets --all-features -- -D warnings`.
   - Run `cargo test --all-features`.
   - Check `git diff` to ensure expected modifications.
   - Address any fallout in `relvar-core/src/query/mod.rs` or tests from the signature change.

5. **Complete pre-commit steps**
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.

6. **Submit PR via `pr.py`**
   - Create `pr_description.md` outlining the optimizations as per Bolt's guidelines.
   - Run `python3 pr.py` to submit the branch `bolt-query-builder-alloc-optimization`.
"""
print(plan)
