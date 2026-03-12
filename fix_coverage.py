import os

with open("lcov.info", "r") as f:
    content = f.read()

# check missing coverage
lines = content.split('\n')
current_file = None
for line in lines:
    if line.startswith("SF:"):
        current_file = line[3:]
    elif line.startswith("DA:") and line.endswith(",0"):
        if current_file and "virtual_relvar.rs" in current_file:
            print(f"Missing coverage in {current_file}: {line}")
