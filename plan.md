1. **Refactor long functions in `relvar/src/experimental` using helper functions/closures to flatten logic**
   - I will extract logic in functions over 50 lines to shorter closures or helper methods.
   - Files to refactor: `relvar/src/experimental/spreadsheet.rs` (evaluate), `relvar/src/experimental/graph.rs` (compute_pagerank_iteration), `relvar/src/experimental/physics.rs` (compute_pairwise_forces), `relvar/src/experimental/neural_network.rs` (forward_layer), `relvar/src/experimental/image.rs` (compute_kernel_contributions), `relvar/src/experimental/genetic_algorithm.rs` (reproduce, select_parents), `relvar/src/experimental/gnn.rs` (forward).
   - I have already successfully tested all these files, reducing their length under 60 lines.

2. **Run pre-commit steps to make sure proper testing, verifications, reviews and reflections are done.**

3. **Submit the PR using `python pr.py`**
   - The commit message will be short and clear.

4. **Conclude the session**
   - Run `python finish_relvar.py`
