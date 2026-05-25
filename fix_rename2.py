import re

def replace_in_file(filepath, search, replace):
    with open(filepath, 'r') as f:
        content = f.read()

    if search in content:
        content = content.replace(search, replace)
        with open(filepath, 'w') as f:
            f.write(content)
        print(f"Replaced in {filepath}")
    else:
        print(f"Not found in {filepath}")

# We're trying to fix this: "error[E0277]: a value of type `BTreeMap<std::string::String, _>` cannot be built from an iterator over elements of type `(&str, values::scalar::ScalarValue)`"
# when trying to optimize rename_into by eliminating the Vec<String> allocs

# Reverting rename changes to get it compiling
rename_revert_1 = """fn build_renamed_heading_and_names<'a>(
    old_heading: &'a TupleType,
    mappings: &'a [(&'a str, &'a str)],
) -> (TupleType, Vec<&'a str>) {"""

rename_revert_rep_1 = """fn build_renamed_heading_and_names(
    old_heading: &TupleType,
    mappings: &[(&str, &str)],
) -> (TupleType, Vec<String>) {"""

rename_revert_2 = """    let mut new_names = Vec::with_capacity(old_heading.degree());

    for (old_name, attr_type) in old_heading.attributes() {
        // Check if this attribute should be renamed
        let new_name = mappings
            .iter()
            .find(|(from, _)| from == old_name)
            .map(|(_, to)| *to)
            .unwrap_or(&old_name.as_str());

        new_heading = new_heading.with_attribute(*new_name, attr_type.clone());
        new_names.push(*new_name);
    }"""

rename_revert_rep_2 = """    let mut new_names = Vec::with_capacity(old_heading.degree());

    for (old_name, attr_type) in old_heading.attributes() {
        // Check if this attribute should be renamed
        let new_name = mappings
            .iter()
            .find(|(from, _)| from == old_name)
            .map(|(_, to)| *to)
            .unwrap_or(old_name.as_str());

        new_heading = new_heading.with_attribute(new_name, attr_type.clone());
        new_names.push(new_name.to_string());
    }"""


replace_in_file('relvar-core/src/algebra/rename.rs', rename_revert_1, rename_revert_rep_1)
replace_in_file('relvar-core/src/algebra/rename.rs', rename_revert_2, rename_revert_rep_2)
