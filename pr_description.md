🚮 Smell: `bench_insert_with_check_constraint` in `benches/database.rs` was over 100 lines long ("God Function") and contained duplicate logic for setting up a temporary database with an `EMP` relation across three different benchmark functions.
✨ Solution: Extracted the common setup logic into a new private helper function `setup_emp_db_with_constraints(constraints: Option<CheckConstraints>) -> (TempDir, Database)`. Used this helper within `simple_check`, `complex_check`, and `no_check`.
🧼 Benefit: Significantly reduces boilerplate and duplicate logic, shortening the `bench_insert_with_check_constraint` function and enforcing the DRY principle. Makes the benchmark setups easier to understand and maintain.
🛡️ Verification: Tests passed. No logic changed. Verified via `git diff`.
