🔒 Warden: Fix Stack Overflow in Deserialization of Query and ConstraintExpression

💡 **The Spark:**
The `Query` and `ConstraintExpression` ASTs use `Box` for recursive variants (e.g. `Join(Box<Query>, Box<Query>)`, `And(Box<ConstraintExpression>, Box<ConstraintExpression>)`). When these are deserialized via `postcard` or other formats without an inherent recursion limit, a deeply nested JSON or binary payload could bypass normal Serde checks and trigger a stack overflow. This represents a Denial of Service (DoS) vulnerability.

🛡️ **Defense:**
By placing `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` on every recursive `Box` field in the AST definitions for `Query` and `ConstraintExpression`, we integrate our thread-local recursion tracking directly into the deserialization process. This limits nesting depth to `MAX_RECURSION_DEPTH` (64) safely intercepting and halting adversarial payloads.

💥 **Severity:**
Critical. A malformed but validly typed deeply recursive query payload could crash the process via stack overflow.

🧪 **Verification:**
Added `security_tests::test_query_postcard_recursion_limit` and `test_constraint_postcard_recursion_limit` tests which generate moderately deep ASTs and verify that they hit the explicit Serde error for recursion limits when deserialized from `postcard` bytes. Tests passed cleanly and formatting/clippy checks were verified.
