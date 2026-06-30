🔒 Warden: Fix Image Exporter Integer Overflow DoS

🦠 Threat: A potential Denial of Service (DoS) vulnerability existed in `relvar::experimental::image::save`. The bounds mapping algorithm relied on `(max_x.saturating_sub(min_x).saturating_add(1)) as usize` to calculate image sizes. Since extreme boundary input cases were not validated properly, unchecked relative coordinates across memory boundary extremes (e.g., `i64::MIN` and `i64::MAX`) could wrap around during casting or trigger memory allocation bypasses and limit exhaustion due to missing safety checks prior to memory vector allocation. Unsafe negative wrap limits across pixel tuples (`(x - min_x) as usize`) also caused integer underflows resulting in panics.

🛡️ Defense: Hardened dimension difference calculation strictly evaluating bounded ranges utilizing `diff_x.checked_add(1)` with safe, explicit `usize::try_from` casting. Implemented strict `min_x > max_x` validation. Pixel index projections mapping coordinates are now hardened with `usize::try_from(x.saturating_sub(min_x))` allowing extreme data input tuples to fail silently with bounds ignoring over panics.

💥 Severity: Medium - A rogue user query could theoretically construct extremely disparate pixel tuples across `i64` boundaries designed to induce unhandled runtime panics when saving relational views as image data.

🧪 Verification: Verified by implementing exhaustive fuzz bounds tests (`warden_exploit_image_overflow.rs`) successfully capturing extreme bounds constraints mapping across `i64::MIN` through `i64::MAX`. Validated fix blocks boundary panics.
