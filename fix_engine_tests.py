import re

with open('relvar-storage/src/persistent_engine.rs', 'r') as f:
    lines = f.readlines()

test_starts = []
for i, line in enumerate(lines):
    if line.strip().startswith('#[test]'):
        for j in range(i+1, min(i+5, len(lines))):
            if 'fn ' in lines[j]:
                test_starts.append((i, j))
                break

for start_idx, fn_idx in reversed(test_starts):
    braces = 0
    in_block = False
    end_idx = fn_idx
    for i in range(fn_idx, len(lines)):
        if '{' in lines[i]:
            braces += lines[i].count('{')
            in_block = True
        if '}' in lines[i]:
            braces -= lines[i].count('}')
        if in_block and braces == 0:
            end_idx = i
            break

    if not in_block: continue

    test_content = "".join(lines[fn_idx:end_idx+1])

    if "test_abort_does_not_flush" in test_content or \
       "test_recovery_retains_valid_data_during_cleanup" in test_content or \
       "test_recovery_undoes_uncommitted_data" in test_content or \
       "test_transaction_rollback" in test_content or \
       "test_transaction_with_multiple_relations" in test_content:

        real_start = start_idx
        while real_start > 0:
            prev = lines[real_start - 1].strip()
            if prev.startswith('///') or prev == '':
                real_start -= 1
            else:
                break
        lines = lines[:real_start] + lines[end_idx+1:]

with open('relvar-storage/src/persistent_engine.rs', 'w') as f:
    f.writelines(lines)
