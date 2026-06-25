I plan to add a new experimental module for evaluating Turing Machines purely via Relational Algebra.

**Module:** `relvar-core/src/experimental/turing_machine.rs`

**Logic:**
1. A Turing machine configuration consists of:
    * `tape`: A relation with `pos` (Int) and `symbol` (String).
    * `head`: A relation with `state` (String) and `pos` (Int) - normally 1 row.
    * `transitions`: A relation with `current_state` (String), `read_symbol` (String), `next_state` (String), `write_symbol` (String), and `move_dir` (Int).
2. The `step` function performs a single step using relational operations (Join, Extend, Union, Difference, Project, Rename):
    * Joins `head` with `tape` to get the current symbol (or defaults to `_` if blank).
    * Joins the result with `transitions` to find the matching rule.
    * Uses `extend` to compute the next head position.
    * Updates the `tape` by taking the `difference` to remove the old symbol and `union`ing the new symbol.
    * Returns the updated `tape` and `head` relations.

**Pre-commit steps:**
- Add tests to prove the logic (e.g. 2-state Busy Beaver).
- Run workspace checks (`cargo test`, `cargo clippy`, `cargo fmt`).
- Maintain journal `.jules/nova.md`

Are there any issues with this plan?
