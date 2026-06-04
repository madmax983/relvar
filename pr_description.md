# Title
🔒 Warden: Prevent stack overflow DoS in recursive AST types

# Description
🦠 **Threat:** Unbounded recursion in `Query` and `ConstraintExpression` enums allowed deeply nested structures to cause stack overflow Denial of Service (DoS) during POSTCARD deserialization.

🛡️ **Defense:** Applied `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` directly to the recursive `Box` fields in both enums, enforcing a maximum recursion depth of 64 limits and safely returning an error on deeply nested inputs.

💥 **Severity:** High - Attackers could crash the application by sending malicious, deeply nested payloads.

🧪 **Verification:** Added fuzzing-style test cases (`tests/warden_exploit_query_recursion.rs`) verifying that deserialization fails cleanly without overflowing the stack. Also ran `cargo test` and `cargo clippy --all-targets --all-features -- -D warnings`.
