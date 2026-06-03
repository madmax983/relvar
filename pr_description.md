🛡️ Sentry: [test coverage improvement]

🎯 Target: Coverage for `relvar/src/tools/exporter.rs`, `relvar/src/tools/importer.rs`, `relvar/src/tools/visualizer.rs`, and `relvar/src/lib.rs`.

💣 Risk: Untested error boundaries and formatting corner cases within tooling such as CSV importing bounds, exporter formatting for nested relations, and diagram generation for primary keys.

🧪 Strategy: Added specific unit tests under `relvar/src/tools/tests.rs` for tool coverage and `relvar/src/lib.rs` for db factory methods.

🔬 Verification: Execute `cargo test -p relvar --lib tools::tests::` and `cargo test -p relvar --lib tests::`
