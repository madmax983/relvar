import sys

def main():
    with open('relvar-core/src/algebra/group.rs', 'r') as f:
        content = f.read()

    old_code = """        // For each tuple in the RVA, create a new tuple combining non-RVA and RVA attributes
        for rva_tuple in rva_relation.tuples() {
            let mut values = std::collections::BTreeMap::new();

            // Add non-RVA attribute values
            for attr_name in relation.relation_type().tuple_type().attribute_names() {
                if attr_name != rva_name {
                    values.insert(attr_name.to_string(), tuple.get(attr_name).unwrap().clone());
                }
            }

            // Add RVA tuple's attribute values
            for attr_name in rva_relation_type.tuple_type().attribute_names() {
                values.insert(
                    attr_name.to_string(),
                    rva_tuple.get(attr_name).unwrap().clone(),
                );
            }

            let result_tuple = Tuple::new_unchecked(result_heading_arc.clone(), values);
            result_tuples.push(result_tuple);
        }"""

    new_code = """        // Cache the non-RVA attribute values for this tuple
        // Instead of re-extracting and cloning them for every tuple in the RVA
        let mut non_rva_values = std::collections::BTreeMap::new();
        for (attr_name, value) in tuple.values() {
            if attr_name != rva_name {
                non_rva_values.insert(attr_name.clone(), value.clone());
            }
        }

        // For each tuple in the RVA, create a new tuple combining non-RVA and RVA attributes
        for rva_tuple in rva_relation.tuples() {
            let mut values = non_rva_values.clone();

            // Add RVA tuple's attribute values
            for (attr_name, value) in rva_tuple.values() {
                values.insert(attr_name.clone(), value.clone());
            }

            let result_tuple = Tuple::new_unchecked(result_heading_arc.clone(), values);
            result_tuples.push(result_tuple);
        }"""

    if old_code in content:
        content = content.replace(old_code, new_code)
        with open('relvar-core/src/algebra/group.rs', 'w') as f:
            f.write(content)
        print("Success")
    else:
        print("Could not find code to replace")

if __name__ == "__main__":
    main()
