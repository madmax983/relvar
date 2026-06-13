import sys
import subprocess

title = '🛡️ Sentry: [test coverage improvement]'
body = '🎯 Target: `Database::delete`, `Database::update`, `Database::create_relvar` error paths.\n💣 Risk: Missing test coverage for error conditions like foreign key violation during delete, type constraint and check constraint violations during update, and naming conflict when creating a virtual relvar. This lack of coverage creates a risk where regression in constraint validation might go unnoticed.\n🧪 Strategy: Added integration tests `test_delete_foreign_key_violation`, `test_update_check_constraint_violation`, `test_update_type_constraint_violation`, and `test_create_relvar_virtual_exists` to comprehensively verify these error scenarios.\n🔬 Verification: Run `cargo test --manifest-path relvar-core/Cargo.toml --test sentry_database_missing_coverage` to verify the tests.'

subprocess.run(["git", "commit", "-am", f"{title}\n\n{body}"])
