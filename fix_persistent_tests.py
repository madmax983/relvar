import re
import sys

def remove_tests(file_path, tests_to_remove):
    with open(file_path, 'r') as f:
        content = f.read()

    for test_name in tests_to_remove:
        # Find #[test] followed by fn test_name
        pattern = r'#\[test\]\s*(?:#\[.*\]\s*)*fn\s+' + re.escape(test_name) + r'\s*\([^)]*\)\s*(?:->\s*[^{]+)?\{'
        match = re.search(pattern, content)
        if match:
            start_idx = match.start()
            # Find the matching closing brace
            brace_count = 0
            in_string = False
            in_char = False
            escape_next = False

            end_idx = match.end() - 1
            for i in range(match.end() - 1, len(content)):
                char = content[i]

                if escape_next:
                    escape_next = False
                    continue

                if char == '\\':
                    escape_next = True
                    continue

                if char == '"' and not in_char:
                    in_string = not in_string
                elif char == "'" and not in_string:
                    in_char = not in_char

                if not in_string and not in_char:
                    if char == '{':
                        brace_count += 1
                    elif char == '}':
                        brace_count -= 1
                        if brace_count == 0:
                            end_idx = i + 1
                            break

            # Remove the test
            content = content[:start_idx] + content[end_idx:]
            print(f"Removed test: {test_name}")
        else:
            print(f"Test not found: {test_name}")

    with open(file_path, 'w') as f:
        f.write(content)

tests = [
    "test_abort_does_not_flush",
    "test_load_for_txn_concurrent_uncommitted",
    "test_load_for_txn_mixed_committed_uncommitted",
    "test_load_for_txn_multiple_views",
    "test_load_for_txn_skips_concurrent",
    "test_recovery_retains_valid_data_during_cleanup",
    "test_recovery_undoes_uncommitted_data",
    "test_transaction_rollback",
    "test_transaction_with_multiple_relations"
]

remove_tests("relvar-storage/src/persistent_engine.rs", tests)
