1. **Refactor `compute_ungrouped_tuples` in `relvar-core/src/algebra/group.rs`**:
   - The `compute_ungrouped_tuples` function is currently 61 lines long, which violates the "God Function" smell.
   - I will extract the logic for extracting the RVA from the tuple into a separate helper function, `extract_rva`, to make it cleaner and shorter.

2. **Run `pre_commit_instructions`**:
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.

3. **Submit**:
   - Create PR using `pr.py` with appropriate branch and title for Forge.
