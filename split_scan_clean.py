import re

with open('relvar-storage/src/storage/heap/tests/scan.rs', 'r') as f:
    content = f.read()

header = """#![allow(unused_imports)]
use super::common::*;
use crate::mvcc::TransactionSnapshot;
use crate::storage::heap::*;
use crate::wal::Lsn;
use relvar_core::tuple;
use relvar_core::types::{ScalarType, TupleType};
use std::collections::HashSet;
use tempfile::NamedTempFile;

"""

def parse_functions(content):
    blocks = []
    current_block = ""
    in_block = False
    brace_count = 0

    # We will split strictly by #[test] and their corresponding blocks
    lines = content.split('\n')
    i = 0
    while i < len(lines):
        line = lines[i]

        if not in_block:
            if line.startswith("#[test]") or line.strip().startswith("fn test_") or line.startswith("mod "):
                in_block = True
                current_block = line + "\n"
                if "{" in line:
                    brace_count += line.count("{") - line.count("}")
                if brace_count == 0 and "{" in line:
                    blocks.append(current_block)
                    in_block = False
                    current_block = ""
        else:
            current_block += line + "\n"
            brace_count += line.count("{") - line.count("}")
            if brace_count == 0:
                blocks.append(current_block)
                in_block = False
                current_block = ""
        i += 1
    return blocks

blocks = parse_functions(content)

# We will collect everything properly
files = {
    'scan_visible': [],
    'scan_corruptions': [],
    'scan_security': [],
    'scan_offset_overflow': [],
    'scan': []
}

for block in blocks:
    if "test_scan_visible" in block:
        files['scan_visible'].append(block)
    elif "corrupt" in block or "allocation_bomb" in block:
        files['scan_corruptions'].append(block)
    elif "mod offset_overflow_tests" in block:
        # Extract inner functions
        inner_funcs = re.findall(r'(#\[test\]\s+fn test_.*?^    })', block, re.MULTILINE | re.DOTALL)
        if not inner_funcs:
             inner_funcs = re.findall(r'(    fn test_.*?^    })', block, re.MULTILINE | re.DOTALL)
        for fn in inner_funcs:
            # unindent
            fn_unindented = "\n".join([line[4:] if line.startswith("    ") else line for line in fn.split('\n')])
            files['scan_offset_overflow'].append(fn_unindented)
    elif "mod security_tests" in block:
        inner_funcs = re.findall(r'(#\[test\]\s+fn test_.*?^    })', block, re.MULTILINE | re.DOTALL)
        if not inner_funcs:
             inner_funcs = re.findall(r'(    fn test_.*?^    })', block, re.MULTILINE | re.DOTALL)
        for fn in inner_funcs:
            # unindent
            fn_unindented = "\n".join([line[4:] if line.startswith("    ") else line for line in fn.split('\n')])
            files['scan_security'].append(fn_unindented)
    elif "overflow" in block or "out_of_bounds" in block:
        files['scan_security'].append(block)
    else:
        files['scan'].append(block)

for f, fblocks in files.items():
    with open(f"relvar-storage/src/storage/heap/tests/{f}.rs", 'w') as out:
        out.write(header + "\n" + "\n".join(fblocks))
