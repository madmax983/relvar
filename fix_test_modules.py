import re

def fix_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    test_module_idx = content.find('#[cfg(test)]')
    if test_module_idx == -1:
        return False

    before_test = content[:test_module_idx]
    test_module = content[test_module_idx:]

    # We added our functions at the end of the file, after the test module
    # Let's find the end of the test module (the last closing brace that isn't indented)
    last_brace_idx = test_module.rfind('}\n')

    if last_brace_idx == -1:
        return False

    actual_test_module = test_module[:last_brace_idx + 2]
    added_functions = test_module[last_brace_idx + 2:]

    if not added_functions.strip():
        return False

    new_content = before_test + added_functions + "\n" + actual_test_module
    with open(filepath, 'w') as f:
        f.write(new_content)
    return True

fix_file("relvar/src/experimental/raytracer.rs")
fix_file("relvar/src/experimental/physics.rs")
fix_file("relvar/src/experimental/neural_network.rs")
fix_file("relvar/src/experimental/image.rs")
fix_file("relvar/src/experimental/spreadsheet.rs")
