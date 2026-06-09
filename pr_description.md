🔒 Warden: [security fix]

🦠 Threat: Unbounded recursion in `Query` and `ConstraintExpression` enums during `postcard` deserialization allows stack overflow Denial of Service (DoS) attacks.
🛡️ Defense: Applied `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` to recursive `Box` fields to strictly bound traversal depth.
💥 Severity: Critical - remote attackers could crash the server process by sending deeply nested payloads.
🧪 Verification: Added `exploit_test` modules simulating stack overflow scenarios. Verified that `postcard::from_bytes` cleanly returns an error instead of aborting.
