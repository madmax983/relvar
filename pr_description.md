🪒 Razor: [refactor key constraint error handling]

💡 **The Spark:** The method `KeyConstraints::would_violate_on_insert` was returning `Result<Option<Vec<String>>, KeyConstraintError>` and using `Ok(Some(...))` to indicate a violation, which is a confusing anti-pattern.
🚀 **The Feature:** Changed the return type to `Result<(), KeyConstraintError>` and made the method directly return `Err(KeyConstraintError::DuplicateKey(...))` on violation.
🔭 **The Potential:** Simplifies error handling and eliminates conceptually confusing uses of `Option` inside `Result`.
⚠️ **Risk:** Minimal, only internal API usage within tests was affected.
