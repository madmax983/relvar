🚮 Smell
The `SudokuSolver::solve` function was an overly long "God Function" (>100 lines) that mixed multiple distinct relational operations—calculating all possibilities, finding invalid outcomes via constraints, and determining cells—into one massive block.

✨ Solution
Refactored `SudokuSolver::solve` using the "Three-Phase Operator" pattern. Extracted the logic into three cleanly named private helper functions: `compute_all_possibilities`, `compute_invalid_possibilities`, and `find_determined_cells`.

🧼 Benefit
Significantly flattens the execution flow, reducing cognitive load and making it much easier to comprehend how the algorithm navigates relational algebra to solve the grid.

🛡️ Verification
`cargo test`, `cargo clippy`, and `cargo fmt` complete with no errors. No behavior changed.
