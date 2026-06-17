🔒 Warden: [security fix] Fix deserialization bomb in Query and ConstraintExpression

🦠 Threat
A deserialization vulnerability existed in the `Query` and `ConstraintExpression` enums. Because they contain deeply recursive fields (`Box<Query>` and `Box<ConstraintExpression>`) and were using the default `#[derive(Deserialize)]`, an attacker could craft a deeply nested JSON payload that would trigger a stack overflow during `serde_json::from_str`, resulting in a Denial of Service.

🛡️ Defense
Replaced the direct `#[derive(Deserialize)]` on `Query` and `ConstraintExpression` with `#[serde(try_from = "...Unchecked")]`. The unchecked enums mirror the original definitions but apply `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` to all recursive `Box` fields. This limits the recursion depth (via `RecursionGuard`) and returns a safe error instead of crashing the process.

💥 Severity
Critical - could panic the server by crashing the process via stack overflow on untrusted JSON payloads.

🧪 Verification
Added fuzzing test case to verify `Query` and `ConstraintExpression` safely return an error rather than stack overflowing when parsing a payload with a depth of 20,000. Verified that `cargo check`, `cargo clippy`, and `cargo test` pass successfully.

Note: Ignored the hardcoded title in the existing `pr.py` script.
