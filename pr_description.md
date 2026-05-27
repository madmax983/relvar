## 🛡️ Sentry: [test coverage improvement]

🎯 **Target:**
- `relvar-core/src/database/data.rs` (DML constraint pathways for parent/child validation)
- `relvar-core/src/constraints/expression.rs` (Boolean evaluation pathways)

💣 **Risk:**
- Updating or deleting parent tuples with referencing foreign keys triggered missing test branches. If referential constraint validation logic silently skipped missing pathways or returned incorrect error mappings, breaking changes could corrupt relations.
- Comparison constraint evaluations for `CmpOp::Lt`, `CmpOp::Le`, `CmpOp::Gt` were entirely untested.

🧪 **Strategy:**
- Wrote full lifecycle tests for updating and deleting parent tuples that violate child foreign key constraints to prove `DatabaseError::Constraint` emits exactly correctly in `tests/data_coverage.rs`.
- Created robust test vectors evaluating different values using `<, <=, >` relational operations mapping exactly to `CmpOp::*` internally inside `tests_expression_coverage.rs`.

🔬 **Verification:**
```bash
cargo test
```
