with open("relvar/src/experimental/timeseries.rs", "r") as f:
    lines = f.read()

lines = lines.replace("for (attr_name, _) in original_heading.attributes().iter()", "for attr_name in original_heading.attributes().keys()")

with open("relvar/src/experimental/timeseries.rs", "w") as f:
    f.write(lines)
