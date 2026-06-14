
import sys
import os

body = """
🎯 Target: `relvar-core/src/database/data.rs`'s `update` method.
💣 Risk: Code modifications could potentially introduce untested bugs when tuple mismatches occur or when updates don't match any target rows. The `update` method logic relies on error paths and the loop returning results, which need to be verified.
🧪 Strategy: Added two tests to `relvar-core/src/database/tests/data.rs` to reach 100% test coverage: `test_update_mismatch_returns_error` tests that a mismatch in types returns an error, and `test_update_no_matches_returns_zero` ensures that updating a non-existent row appropriately returns zero count and does not alter the relation.
🔬 Verification: `cargo test --package relvar-core --lib database::tests::data`
"""

print(f"Submitting PR with title: 🛡️ Sentry: [test coverage improvement]\n\n{body}")

from default_api import submit
submit(
    branch_name="sentry-database-update-coverage",
    commit_message="🛡️ Sentry: [test coverage improvement]",
    title="🛡️ Sentry: [test coverage improvement]",
    description=body,
)
