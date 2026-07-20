🚮 Smell: The `from_csv` function in `relvar/src/tools/importer.rs` was an overgrown "God Function" (over 120 lines) containing deeply nested loops and mixed logic for reading lines, validating headers, and parsing fields into tuples.
✨ Solution: Extracted `read_line_safe`, `validate_csv_headers`, and `build_tuple_from_csv_row` into focused helper functions to flatten the structure and separate concerns.
🧼 Benefit: Significantly reduces cognitive load. The main loop is now much cleaner and strictly delegates data mapping to its helpers.
🛡️ Verification: Tests passed. No logic changed.
