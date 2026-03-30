1. **Optimize Foreign Key `would_violate_on_insert` and `would_violate_on_delete`**
   - Replace intermediate `Vec<_>` collections with `Vec<&ScalarValue>` using references to avoid allocating completely new `Vec<ScalarValue>` for every foreign key check.
   - Refactor `would_violate_on_insert` to use zero-copy checking.
   - Refactor `would_violate_on_delete` to use zero-copy checking.

2. **Optimize `CandidateKey`'s `is_satisfied_by` and `would_violate`**
   - Refactor `extract_key_value` to `extract_key_value_ref` returning `Result<Vec<&crate::values::ScalarValue>, KeyConstraintError>` to eliminate `val.clone()` inside the loop for constraint checks.
   - Update `is_satisfied_by` to collect `Vec<&ScalarValue>` into the `HashSet`.
   - Update `would_violate` to compare `Vec<&ScalarValue>`.
   - Keep the original `extract_key_value` or replace it if not used elsewhere.

3. **Complete pre commit steps**
   - Complete pre commit steps to ensure proper testing, verification, review, and reflection are done.

4. **Submit the change**
   - Once all tests pass, submit the change with a descriptive commit message.
