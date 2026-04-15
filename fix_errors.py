import re
import subprocess
import json

def run_cargo_check():
    result = subprocess.run(['cargo', 'check', '--all-targets', '--all-features', '--message-format=json'], capture_output=True, text=True)
    return result.stdout.split('\n')

def fix_errors():
    max_iters = 10
    for _ in range(max_iters):
        print("Running cargo check...")
        output = run_cargo_check()
        fixes = 0
        for line in output:
            if not line: continue
            try:
                msg = json.loads(line)
                if msg.get('reason') == 'compiler-message' and msg['message']['level'] == 'error':
                    spans = msg['message']['spans']
                    for span in spans:
                        if span['is_primary']:
                            file_path = span['file_name']
                            line_num = span['line_start']
                            col_start = span['column_start']
                            col_end = span['column_end']
                            text = span['text']

                            with open(file_path, 'r') as f:
                                lines = f.readlines()

                            if line_num <= len(lines):
                                line_content = lines[line_num - 1]
                                error_msg = msg['message']['message']

                                if "expected `PageId`, found integer" in error_msg or "expected `PageId`, found `u64`" in error_msg:
                                    span_text = line_content[col_start-1:col_end-1]

                                    # Very naive wrap
                                    if span_text.strip().isdigit() or span_text.strip() in ["i", "page_id", "page_id.0"]:
                                        if span_text.strip() == "page_id.0":
                                            new_line = line_content[:col_start-1] + f"page_id" + line_content[col_end-1:]
                                        else:
                                            new_line = line_content[:col_start-1] + f"PageId({span_text})" + line_content[col_end-1:]
                                        lines[line_num - 1] = new_line
                                        fixes += 1
                                    elif "0" in span_text:
                                        new_line = line_content[:col_start-1] + f"PageId({span_text})" + line_content[col_end-1:]
                                        lines[line_num - 1] = new_line
                                        fixes += 1
                                    elif "1" in span_text or "5" in span_text or "42" in span_text or "999" in span_text or "1000" in span_text:
                                        new_line = line_content[:col_start-1] + f"PageId({span_text})" + line_content[col_end-1:]
                                        lines[line_num - 1] = new_line
                                        fixes += 1

                            with open(file_path, 'w') as f:
                                f.writelines(lines)
            except json.JSONDecodeError:
                pass

        if fixes == 0:
            break
        print(f"Applied {fixes} fixes.")

fix_errors()
