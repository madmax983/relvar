🔒 Warden: Fix integer overflow and logic flaws in blockchain

🦠 Threat: Integer overflow via `i64::MIN.saturating_neg()` returning `i64::MAX`, leading to logic errors in balance calculation. Additionally, a logic flaw allowed processing of negative transaction amounts, potentially enabling unauthorized creation of funds (e.g. Alice sending -100 to Bob increases Alice's balance by 100).
🛡️ Defense: Replaced `saturating_neg()` with `checked_neg().unwrap_or(0)` to prevent wrapping at `i64::MIN`. Added a validation step in `is_valid()` to restrict and reject any transaction with an amount less than 0.
💥 Severity: High - could allow attackers to bypass standard balance validations and mint arbitrary funds by exploiting signed integer wrap-arounds and logic flaws.
🧪 Verification: Added fuzzing test cases to assert safe panic/default and prevent logic abuse.
