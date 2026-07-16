🔒 Warden: [security fix] Integer Overflow DoS in Convolution Filters

🦠 **Threat**: The `apply_kernel` function in `relvar::experimental::image` performed unchecked arithmetic operations (`+` and `*`) when calculating target coordinates and weighted color components. A malicious payload with extreme pixel positions or high-weight kernel taps could trigger an integer overflow panic on coordinates, crashing the server processing the user's images.

🛡️ **Defense**: Switched to using safe bounded arithmetic via `saturating_add` and `saturating_mul` for processing pixel coordinates and kernel weight values, avoiding panics across convolution filter operations.

💥 **Severity**: High - Remote execution of unbounded image data convolution can easily panic the Rust program, resulting in a Denial of Service (DoS).

🧪 **Verification**: Added `test_image_kernel_overflow_dos` verification in `tests/warden_exploit_image_kernel_dos.rs` to fuzz kernel and payload variables and prove the operations are now properly handled without a panic.
