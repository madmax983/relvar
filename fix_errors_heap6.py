with open('relvar-storage/src/storage/heap.rs', 'r') as f:
    lines = f.readlines()

for i, line in enumerate(lines):
    if "self.page_file.read_page(page_id)?" in line:
        # Check if page_id is a u64 in this scope
        # By looking at previous 15 lines
        prev_lines = "".join(lines[max(0, i-15):i])
        if "page_id: u64" in prev_lines or "= 0;" in prev_lines or "for page_id in 0.." in prev_lines or "let page_id = current_page" in prev_lines:
            lines[i] = line.replace("read_page(page_id)", "read_page(PageId(page_id))")

with open('relvar-storage/src/storage/heap.rs', 'w') as f:
    f.writelines(lines)
