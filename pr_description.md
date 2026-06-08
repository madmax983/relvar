🔒 Warden: [security fix]

🦠 Threat: Unbounded recursion in `Query` and `ConstraintExpression` ASTs allowing stack overflow Denial of Service during deserialization of crafted payloads.
🛡️ Defense: Applied `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` to the recursive `Box` fields to enforce the `MAX_RECURSION_DEPTH` guard during parsing.
💥 Severity: Critical - could panic and crash the database server process.
🧪 Verification: Added fuzzing-style exploit tests `warden_exploit_query_expr_recursion.rs` that verify the stack overflow is prevented and instead safely returns an Err.
