🔒 Warden: Fix DoS Vulnerabilities via Integer Overflows in Image Coordinates and WAL Metadata

🦠 **Threat:**
1. **Memory Exhaustion DoS in Image Parsing:** The `image::save` feature in `relvar` blindly parsed unbounded `max_x` and `max_y` tuples and performed `(max_x - min_x) as usize`, directly propagating unchecked coordinates from relational inputs into unbounded memory allocations (`vec![0u8; width * height * 3]`), leading to a trivial out-of-memory DoS condition.
2. **Metadata Length Bypass in Heap/WAL System:** Serialization parsers inside `relvar-storage` manually parsed and blindly bypassed prefix validations, performing unvalidated casts like `u32::MAX as usize`. On 32-bit compilation targets or within heavily restricted environments, an intentionally malformed WAL binary could crash the process unconditionally prior to memory saturation checks.

🛡️ **Defense:**
- Stripped all `as usize` operations in the Image pipeline. Enforced explicit `usize::try_from` mappings coupled with safe `checked_add`/`checked_mul` chains, causing any extreme integer bounds supplied to cleanly overflow into truncated conditions and naturally short-circuit before memory assignment.
- Removed arbitrary memory allocations that relied on `u32::MAX as usize` in `relvar-storage`, enforcing a hard `try_from` check that handles architecture boundary limits efficiently.

💥 **Severity:**
Critical. Allows direct memory DoS and unhandled out-of-bounds process crashing from both external file processing and database queries pushing malformed tuples.

🧪 **Verification:**
Added `warden_exploit_image_dos.rs` to demonstrate that placing MAX limits in images resolves to empty vectors rather than panics. Verified `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test` confirm sound constraints.
