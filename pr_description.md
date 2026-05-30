# Title
🔒 Warden: [Fix recursion DoS in ConstraintExpression deserialization]

# Description
🦠 Threat: Unbounded recursion (stack overflow Denial of Service) through recursive deserialization via serde (`And`, `Or`, `Not` enum variants in `ConstraintExpression`).
🛡️ Defense: Added `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` to the `Box<ConstraintExpression>` fields within the recursive enum variants, applying the existing bounds checker guard.
💥 Severity: High - A malicious user could provide a deeply nested logic constraint JSON string payload that would exhaust the stack and crash the system.
🧪 Verification: Created testing exploit to verify the vulnerability existed and verify the system correctly panicked without crashing after the patch was applied using standard tests `cargo test`.
