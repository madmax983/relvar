import sys

def parse_mod_visibility(filepath):
    print(f"File: {filepath}")
    with open(filepath, 'r') as f:
        for line in f:
            if "pub mod" in line or "pub(crate) mod" in line:
                print(line.strip())

parse_mod_visibility("relvar/src/lib.rs")
