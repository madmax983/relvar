# The warnings are mostly about `never used` things in `relvar/src/experimental/*`.
# Because I'm in strict mode and trying to fix the CI failure.
# The CI failed because there's a `-D warnings` flag, and I have a bunch of unused structs/functions
# that are triggering warnings. But wait! I didn't introduce all these warnings. Why are they failing now?
# The errors only started showing up because maybe a test module or `pub mod` statement was changed?

# Ah! `relvar/src/lib.rs:149: pub(crate) mod experimental;`
# But the test files in `tests/` like `sentry_timeseries_collision.rs` are external tests. They cannot access `pub(crate) mod experimental`.
# Wait, let's check `relvar/src/lib.rs`
