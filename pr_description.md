🎯 Target: `relvar-core/src/database/data.rs` DML API facade constraint validation error paths.
💣 Risk: Untested constraint failure paths in critical `delete`, `update`, and `insert` database boundary methods could mask bugs when cascading updates or inserts violate data integrity rules.
🧪 Strategy: Added specific unit tests triggering bulk key violations, insert type mismatch errors, and referencing foreign key failures on update/delete to map specifically to those untested error bubbling branches (covering lines `153, 156-158, 260, 265-267, 297-299, 305-307, 328-330` in `data.rs`).
🔬 Verification: `cargo test -p relvar-core --tests sentry_coverage`
