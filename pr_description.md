Title: 🔒 Warden: Prevent Stack Overflow DoS in Query and ConstraintExpression Deserialization

🦠 **Threat**: Stack overflow Denial of Service (DoS). The `Query` and `ConstraintExpression` enums contained recursive `Box` fields (`input`, `left`, `right`, `And`, `Or`, `Not`) that were deserialized directly using `serde`. While `serde_json` has a safe default recursion limit, the `postcard` format used by the engine has no such limit, allowing an attacker to submit deeply-nested structures that crash the database process via a stack overflow.

🛡️ **Defense**: Applied `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` to all recursive `Box` fields in `Query` and `ConstraintExpression` to track and safely cap deserialization depth, returning an error when the limit is breached.

💥 **Severity**: Critical. Exploitation takes minimal effort and leads to an immediate crash of the database process (Availability impact).

🧪 **Verification**: Added `warden_exploit_query_recursion` and `warden_exploit_constraint_recursion` tests that build deeply nested structures and ensure `postcard` deserialization gracefully returns a recursion limit error rather than overflowing the stack. All tests pass and `cargo clippy --all-targets --all-features -- -D warnings` reports no warnings.
