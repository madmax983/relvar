# 🛡️ Sentry: [test coverage improvement]

## 🎯 Target
- `relvar-storage/src/storage/page.rs` (Page creation, parsing, and buffered writes)

## 💣 Risk
Untested error handling boundaries for serialization limitations, mismatched offsets, and out-of-bounds page sizes, which could cause silent corruption or panics if malformed structures bypass standard validation.

## 🧪 Strategy
Added specific edge case tests targeting:
- `Page::set_data` limits (`test_page_set_data_too_large`)
- Combined bounds errors between `set_data` and write operations causing prefix overflow (`test_page_write_too_large_via_set_data`)
- Deserialization corruption with misleading block lengths (`test_page_parse_corrupted_length`)

## 🔬 Verification
Run `cargo test --package relvar-storage test_page_set_data_too_large` and `cargo llvm-cov` to verify coverage.
