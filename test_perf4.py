import sys

def main():
    with open('relvar-core/src/algebra/group.rs', 'r') as f:
        content = f.read()

    old_code = """fn compute_ungrouped_tuples(
    relation: &Relation,
    rva_name: &str,
    result_heading: &TupleType,
    _rva_relation_type: &RelationType,
) -> Result<impl Iterator<Item = Tuple> + '_, UngroupError> {
    let result_heading_arc = std::sync::Arc::new(result_heading.clone());

    // We can avoid intermediate Vec allocation entirely by returning an iterator
    // that yields ungrouped tuples on the fly.
    // Flat map over the original tuples
    let iter = relation.tuples().flat_map(move |tuple| {
        // Extract the RVA relation from the tuple
        let rva_relation = match tuple.get(rva_name).unwrap() {
            ScalarValue::Relation(rel) => rel,
            _ => panic!("Expected relation value"), // Validated upstream
        };

        // Cache the non-RVA attributes for this specific group to avoid N*M re-extractions
        let mut non_rva_values = std::collections::BTreeMap::new();
        for (attr_name, value) in tuple.values() {
            if attr_name != rva_name {
                non_rva_values.insert(attr_name.clone(), value.clone());
            }
        }

        let heading_arc = result_heading_arc.clone();

        // Map over the RVA tuples and combine them with the non-RVA attributes
        rva_relation.tuples().map(move |rva_tuple| {
            // Clone the cached BTreeMap containing all non-RVA attributes
            let mut combined_values = non_rva_values.clone();

            // Insert all the RVA tuple attributes
            for (attr_name, value) in rva_tuple.values() {
                combined_values.insert(attr_name.clone(), value.clone());
            }

            // Create the resulting tuple without further validation overhead
            Tuple::new_unchecked(heading_arc.clone(), combined_values)
        }).collect::<Vec<_>>()
    }).flatten();

    Ok(iter)
}"""

    new_code = """fn compute_ungrouped_tuples<'a>(
    relation: &'a Relation,
    rva_name: &'a str,
    result_heading: &'a TupleType,
    _rva_relation_type: &'a RelationType,
) -> Result<impl Iterator<Item = Tuple> + 'a, UngroupError> {
    let result_heading_arc = std::sync::Arc::new(result_heading.clone());

    // We can avoid intermediate Vec allocation entirely by returning an iterator
    // that yields ungrouped tuples on the fly.
    // Flat map over the original tuples
    let iter = relation.tuples().flat_map(move |tuple| {
        // Extract the RVA relation from the tuple
        let rva_relation = match tuple.get(rva_name).unwrap() {
            ScalarValue::Relation(rel) => rel,
            _ => panic!("Expected relation value"), // Validated upstream
        };

        // Cache the non-RVA attributes for this specific group to avoid N*M re-extractions
        let mut non_rva_values = std::collections::BTreeMap::new();
        for (attr_name, value) in tuple.values() {
            if attr_name != rva_name {
                non_rva_values.insert(attr_name.clone(), value.clone());
            }
        }

        let heading_arc = result_heading_arc.clone();

        // Map over the RVA tuples and combine them with the non-RVA attributes
        rva_relation.tuples().map(move |rva_tuple| {
            // Clone the cached BTreeMap containing all non-RVA attributes
            let mut combined_values = non_rva_values.clone();

            // Insert all the RVA tuple attributes
            for (attr_name, value) in rva_tuple.values() {
                combined_values.insert(attr_name.clone(), value.clone());
            }

            // Create the resulting tuple without further validation overhead
            Tuple::new_unchecked(heading_arc.clone(), combined_values)
        }).collect::<Vec<_>>().into_iter()
    });

    Ok(iter)
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
