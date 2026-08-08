import re

with open("relvar/src/experimental/timeseries.rs", "r") as f:
    content = f.read()

content = content.replace(
    "for (attr_name, _) in original_heading.attributes().iter() {",
    "for attr_name in original_heading.attributes().keys() {"
)

with open("relvar/src/experimental/timeseries.rs", "w") as f:
    f.write(content)
