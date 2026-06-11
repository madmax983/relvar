🔒 Warden: [security fix]

🦠 Threat: Stack overflow Denial of Service (DoS) vulnerability. Deeply nested recursive AST elements (such as `Query` and `ConstraintExpression`) were being deserialized by `postcard` without any default depth limit, which allowed an attacker to craft deeply nested tree structures to crash the program by exhausting stack space.
🛡️ Defense: Applied `#[serde(deserialize_with = "crate::utils::recursion::deserialize_guarded")]` directly to the recursive `Box` fields for both the `Query` and `ConstraintExpression` enums. This correctly enforces the `MAX_RECURSION_DEPTH` check during deserialization, securely returning an error instead of causing a stack overflow panic.
💥 Severity: High - Unauthenticated crash via DoS.
🧪 Verification: Created `test_query_recursion` and `test_constraint_recursion` that generate inputs exceeding the recursion limit, and assert that deserialization gracefully returns a `Result::Err` instead of crashing.
