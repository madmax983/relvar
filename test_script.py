import sys

def format_rust():
    with open("relvar-core/src/algebra/group.rs", "r") as f:
        content = f.read()

    old_block = """        // Pre-compute the invariant non-RVA attributes for this outer tuple
        let mut base_values = std::collections::BTreeMap::new();
        for (attr_name, val) in tuple.values().iter() {
            if attr_name != rva_name {
                base_values.insert(attr_name.clone(), val.clone());
            }
        }

        // For each tuple in the RVA, create a new tuple combining non-RVA and RVA attributes
        let heading_arc = result_heading_arc.clone();
        for rva_tuple in rva_relation.tuples() {
            let mut values = base_values.clone();
            for (attr_name, val) in rva_tuple.values().iter() {
                values.insert(attr_name.clone(), val.clone());
            }"""

    new_block = """        // Pre-compute the invariant non-RVA attributes for this outer tuple
        let base_values: std::collections::BTreeMap<_, _> = tuple
            .values()
            .iter()
            .filter(|(attr_name, _)| *attr_name != rva_name)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // For each tuple in the RVA, create a new tuple combining non-RVA and RVA attributes
        let heading_arc = result_heading_arc.clone();
        for rva_tuple in rva_relation.tuples() {
            let values: std::collections::BTreeMap<_, _> = base_values
                .clone()
                .into_iter()
                .chain(
                    rva_tuple
                        .values()
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone())),
                )
                .collect();"""

    if old_block in content:
        with open("relvar-core/src/algebra/group.rs", "w") as f:
            f.write(content.replace(old_block, new_block))
        print("Success")
    else:
        print("Failed to find block")

format_rust()
