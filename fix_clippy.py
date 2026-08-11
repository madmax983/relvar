import os

filepath = "relvar/src/experimental/timeseries.rs"
with open(filepath, 'r') as f:
    content = f.read()

content = content.replace("for (attr_name, _) in original_heading.attributes().iter() {", "for attr_name in original_heading.attributes().keys() {")

with open(filepath, 'w') as f:
    f.write(content)
print("Fixed clippy warnings")
