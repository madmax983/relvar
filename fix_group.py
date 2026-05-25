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

# group.rs
group_search_1 = """fn validate_group_request(
    relation: &Relation,
    attrs_to_group: &[&str],
    rva_name: &str,
) -> Result<Vec<String>, GroupError> {"""

group_replace_1 = """fn validate_group_request<'a>(
    relation: &'a Relation,
    attrs_to_group: &[&str],
    rva_name: &str,
) -> Result<Vec<&'a str>, GroupError> {"""

group_search_2 = """    // Attributes that define the groups (those NOT being grouped)
    let grouping_attrs: Vec<String> = relation
        .relation_type()
        .heading()
        .attribute_names()
        .filter(|attr| !attrs_to_group.contains(&attr.as_str()))
        .map(|s| s.to_string())
        .collect();"""

group_replace_2 = """    // Attributes that define the groups (those NOT being grouped)
    let grouping_attrs: Vec<&'a str> = relation
        .relation_type()
        .heading()
        .attribute_names()
        .filter(|attr| !attrs_to_group.contains(&attr.as_str()))
        .map(|s| s.as_str())
        .collect();"""

group_search_3 = """fn build_group_result_heading(
    relation: &Relation,
    grouping_attrs: &[String],
    attrs_to_group: &[&str],
    rva_name: &str,
) -> Result<(TupleType, TupleType), GroupError> {"""

group_replace_3 = """fn build_group_result_heading(
    relation: &Relation,
    grouping_attrs: &[&str],
    attrs_to_group: &[&str],
    rva_name: &str,
) -> Result<(TupleType, TupleType), GroupError> {"""

group_search_4 = """fn compute_grouped_tuples(
    relation: &Relation,
    grouping_attrs: &[String],
    attrs_to_group: &[&str],
    result_heading: &TupleType,
    rva_heading: &TupleType,
    rva_name: &str,
) -> Result<std::collections::HashSet<Tuple>, GroupError> {"""

group_replace_4 = """fn compute_grouped_tuples(
    relation: &Relation,
    grouping_attrs: &[&str],
    attrs_to_group: &[&str],
    result_heading: &TupleType,
    rva_heading: &TupleType,
    rva_name: &str,
) -> Result<std::collections::HashSet<Tuple>, GroupError> {"""


group_search_5 = """    let mut values = std::collections::BTreeMap::new();

        // Add grouping attribute values
        for (i, attr) in grouping_attrs.iter().enumerate() {
            values.insert(attr.clone(), key[i].clone());
        }"""

group_replace_5 = """    let mut values = std::collections::BTreeMap::new();

        // Add grouping attribute values
        for (i, attr) in grouping_attrs.iter().enumerate() {
            values.insert(attr.to_string(), key[i].clone());
        }"""

group_search_6 = """fn group_tuples(
    relation: &Relation,
    grouping_attrs: &[String],
    attrs_to_group: &[&str],
    rva_heading_arc: std::sync::Arc<TupleType>,
) -> Result<HashMap<Vec<crate::values::ScalarValue>, std::collections::HashSet<Tuple>>, GroupError> {"""

group_replace_6 = """fn group_tuples(
    relation: &Relation,
    grouping_attrs: &[&str],
    attrs_to_group: &[&str],
    rva_heading_arc: std::sync::Arc<TupleType>,
) -> Result<HashMap<Vec<crate::values::ScalarValue>, std::collections::HashSet<Tuple>>, GroupError> {"""

group_search_7 = """    // Pre-allocate attribute name strings
    let attr_names: Vec<String> = attrs_to_group.iter().map(|a| a.to_string()).collect();"""

group_replace_7 = """"""

group_search_8 = """        for (i, attr_name) in attr_names.iter().enumerate() {
            let val = tuple
                .get(attr_name)
                .ok_or_else(|| GroupError::MissingAttribute(attr_name.clone()))?;
            rva_values.insert(attr_name.clone(), val.clone());
        }"""

group_replace_8 = """        for (i, attr_name) in attrs_to_group.iter().enumerate() {
            let val = tuple
                .get(attr_name)
                .ok_or_else(|| GroupError::MissingAttribute(attr_name.to_string()))?;
            rva_values.insert(attr_name.to_string(), val.clone());
        }"""


replace_in_file('relvar-core/src/algebra/group.rs', group_search_1, group_replace_1)
replace_in_file('relvar-core/src/algebra/group.rs', group_search_2, group_replace_2)
replace_in_file('relvar-core/src/algebra/group.rs', group_search_3, group_replace_3)
replace_in_file('relvar-core/src/algebra/group.rs', group_search_4, group_replace_4)
replace_in_file('relvar-core/src/algebra/group.rs', group_search_5, group_replace_5)
replace_in_file('relvar-core/src/algebra/group.rs', group_search_6, group_replace_6)
replace_in_file('relvar-core/src/algebra/group.rs', group_search_7, group_replace_7)
replace_in_file('relvar-core/src/algebra/group.rs', group_search_8, group_replace_8)
