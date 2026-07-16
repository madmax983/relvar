🎯 Target: Consuming and Borrowing `TryFrom<ScalarValue>` implementations for `i64`, `f64`, `String`, `bool`, and `Vec<u8>` in `relvar-core/src/values/tuple.rs`.
💣 Risk: Success conversions for both consuming and borrowing were entirely untested. This ensures `std::mem::take` and direct value unwraps inside conversions behave correctly, closing coverage gaps for fundamental extraction traits.
🧪 Strategy: Added direct success assertions for both consuming `TryFrom<ScalarValue>` and borrowing `TryFrom<&ScalarValue>` implementations.
🔬 Verification: `cargo test -p relvar-core --test sentry_tuple_coverage`
