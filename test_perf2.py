import sys

def main():
    with open('relvar-core/src/algebra/group.rs', 'r') as f:
        content = f.read()

    old_code = """        let non_rva_values: Vec<_> = tuple.values()
            .iter()
            .filter(|(k, _)| *k != rva_name)
            .collect();

        // For each tuple in the RVA, create a new tuple combining non-RVA and RVA attributes
        for rva_tuple in rva_relation.tuples() {
            let mut values = std::collections::BTreeMap::new();

            // Add non-RVA attributes
            for (attr_name, value) in &non_rva_values {
                values.insert((*attr_name).clone(), (*value).clone());
            }

            // Add RVA tuple's attribute values
            for (attr_name, value) in rva_tuple.values() {
                values.insert(attr_name.clone(), value.clone());
            }

            let result_tuple = Tuple::new_unchecked(result_heading_arc.clone(), values);
            result_tuples.push(result_tuple);
        }"""

    new_code = """        // Extend non-RVA attribute values and RVA tuples using iterator chains and mapping.
        // BTreeMap::from_iter is faster than repetitive .insert() calls
        let non_rva_values: Vec<_> = tuple.values()
            .iter()
            .filter(|(k, _)| *k != rva_name)
            .collect();

        // For each tuple in the RVA, create a new tuple combining non-RVA and RVA attributes
        for rva_tuple in rva_relation.tuples() {
            let values_iter = non_rva_values.iter()
                .map(|(k, v)| ((*k).clone(), (*v).clone()))
                .chain(rva_tuple.values().iter().map(|(k, v)| (k.clone(), v.clone())));

            let values = std::collections::BTreeMap::from_iter(values_iter);

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
