with open("relvar/src/experimental/timeseries.rs", "r") as f:
    lines = f.readlines()

with open("relvar/src/experimental/timeseries.rs", "w") as f:
    for line in lines:
        if "for (attr_name, _) in original_heading.attributes().iter()" in line:
            f.write(line.replace("for (attr_name, _) in original_heading.attributes().iter()", "for attr_name in original_heading.attributes().keys()"))
        else:
            f.write(line)
