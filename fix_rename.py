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

rename_search_1 = """fn build_renamed_heading_and_names(
    old_heading: &TupleType,
    mappings: &[(&str, &str)],
) -> (TupleType, Vec<String>) {"""

rename_replace_1 = """fn build_renamed_heading_and_names<'a>(
    old_heading: &'a TupleType,
    mappings: &'a [(&'a str, &'a str)],
) -> (TupleType, Vec<&'a str>) {"""

rename_search_2 = """    let mut new_names = Vec::with_capacity(old_heading.degree());

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

rename_replace_2 = """    let mut new_names = Vec::with_capacity(old_heading.degree());

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

replace_in_file('relvar-core/src/algebra/rename.rs', rename_search_1, rename_replace_1)
replace_in_file('relvar-core/src/algebra/rename.rs', rename_search_2, rename_replace_2)
