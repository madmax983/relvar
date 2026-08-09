🦠 **Threat:** `cargo audit` reported two vulnerabilities:
- `RUSTSEC-2026-0204`: Invalid pointer dereference in `crossbeam-epoch`'s `fmt::Pointer` implementation.
- `RUSTSEC-2026-0190`: Unsoundness in `anyhow`'s `Error::downcast_mut()`.

🛡️ **Defense:** Ran `cargo update -p crossbeam-epoch` and `cargo update -p anyhow` to bump both packages to safe versions.

💥 **Severity:** High - potential invalid pointer dereference and memory unsoundness which could lead to UB.

🧪 **Verification:** `cargo audit` now returns a clean bill of health. All tests, formatting, and linting checks continue to pass successfully.
