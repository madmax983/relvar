content = open("relvar-core/src/algebra/group.rs", "r").read()

old_code = """
        // Extract grouped attributes for RVA
        let mut rva_values = std::collections::BTreeMap::new();
        for (i, &attr) in attrs_to_group.iter().enumerate() {
            rva_values.insert(attr_names[i].clone(), tuple.get(attr).unwrap().clone());
        }

        let rva_tuple = Tuple::new_unchecked(rva_heading_arc.clone(), rva_values);
"""

new_code = """
        // Extract grouped attributes for RVA
        let rva_values = std::collections::BTreeMap::from_iter(
            attrs_to_group.iter().enumerate().map(|(i, &attr)| {
                (attr_names[i].clone(), tuple.get(attr).unwrap().clone())
            })
        );

        let rva_tuple = Tuple::new_unchecked(rva_heading_arc.clone(), rva_values);
"""

if old_code in content:
    with open("relvar-core/src/algebra/group.rs", "w") as f:
        f.write(content.replace(old_code, new_code))
    print("Success")
else:
    print("Old code not found")
