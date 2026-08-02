Title: ⚒️ Forge: [Refactor: Extract compute_invalid helper in Sudoku solver]

🚮 Smell: `compute_invalid_possibilities` in `relvar/src/experimental/sudoku.rs` has repetitive logic to join known values against possibilities to determine invalid cells based on row, column, and box constraints.
✨ Solution: Extracted a `compute_invalid` closure internally to DRY up the three identical operations, significantly reducing boilerplate and nesting.
🧼 Benefit: Code length and duplication is heavily reduced, improving readability and maintainability.
🛡️ Verification: Tests passed. No logic changed.
