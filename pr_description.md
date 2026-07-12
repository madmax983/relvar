🦠 Threat: Arithmetic overflow panics in image convolution logic (`total_weight` sum, `x+dx`/`y+dy`, `v*weight`, and division by `-1`) causing a Denial of Service (DoS).
🛡️ Defense: Switched to safe `saturating_*` math and added division overflow guards.
💥 Severity: High - Unauthenticated user input with crafted dimensions or kernel taps could crash the application process via panic.
🧪 Verification: Added comprehensive fuzzer-style test cases (`warden_exploit_apply_kernel*`) demonstrating panics are mitigated under all boundary conditions.
