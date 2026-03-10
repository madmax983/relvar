with open("relvar-core/src/database/tests.rs", "r") as f:
    lines = f.readlines()

# Remove the duplicates at the top that were blindly added by sed
with open("relvar-core/src/database/tests.rs", "w") as f:
    f.writelines(lines[2:])
