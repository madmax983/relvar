1. **Optimize `Query::Rename` Evaluation (`relvar-core/src/query/mod.rs`)**
   - In `Query::execute`, the `Query::Rename` variant currently calls `relation.rename(&mappings_ref)`, which allocates a new set of tuples because `rename` takes `&self` and maps values by cloning.
   - We own `relation` inside the executor (it was returned by `input.execute(db)?`), so we should call `relation.rename_into(&mappings_ref)` instead, taking advantage of the zero-cost abstraction for `rename_into` which reuses allocations and moves values rather than cloning them.

2. **Run verification commands**
   - Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` to ensure tests pass.

3. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.**
   - Run `python pre_commit.sh` and perform cleanup.

4. **Submit PR**
   - Run `python pr.py` to create the PR, title "⚡ Bolt: [Optimization] Use rename_into for Query::Rename execution".

5. **Finish**
   - Run `python finish_relvar.py`
