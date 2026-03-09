import sys

def main():
    with open('relvar-core/src/algebra/group.rs', 'r') as f:
        content = f.read()

    old_code = """fn compute_ungrouped_tuples(
    relation: &Relation,
    rva_name: &str,
    result_heading: &TupleType,
    rva_relation_type: &RelationType,
) -> Result<Vec<Tuple>, UngroupError> {
    let mut result_tuples = Vec::new();
    let result_heading_arc = std::sync::Arc::new(result_heading.clone());

    for tuple in relation.tuples() {
        // Get the RVA relation
        let rva_relation = match tuple.get(rva_name).unwrap() {
            ScalarValue::Relation(rel) => rel,
            _ => return Err(UngroupError::NotRelationValued(rva_name.to_string())),
        };

        // For each tuple in the RVA, create a new tuple combining non-RVA and RVA attributes
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
        }
    }

    Ok(result_tuples)
}"""

    new_code = """fn compute_ungrouped_tuples(
    relation: &Relation,
    rva_name: &str,
    result_heading: &TupleType,
    _rva_relation_type: &RelationType,
) -> Result<Vec<Tuple>, UngroupError> {
    // Estimate capacity based on average RVA cardinality to reduce reallocations
    let mut estimated_capacity = relation.cardinality();
    if relation.cardinality() > 0 {
        if let ScalarValue::Relation(rel) = relation.tuples().next().unwrap().get(rva_name).unwrap() {
            estimated_capacity *= rel.cardinality().max(1);
        }
    }

    let mut result_tuples = Vec::with_capacity(estimated_capacity);
    let result_heading_arc = std::sync::Arc::new(result_heading.clone());

    for tuple in relation.tuples() {
        // Get the RVA relation
        let rva_relation = match tuple.get(rva_name).unwrap() {
            ScalarValue::Relation(rel) => rel,
            _ => return Err(UngroupError::NotRelationValued(rva_name.to_string())),
        };

        // Cache the non-RVA attributes for this specific group to avoid extracting
        // them from the tuple N times where N is the RVA cardinality.
        let mut non_rva_values = std::collections::BTreeMap::new();
        for (attr_name, value) in tuple.values() {
            if attr_name != rva_name {
                non_rva_values.insert(attr_name.clone(), value.clone());
            }
        }

        // For each tuple in the RVA, create a new tuple combining non-RVA and RVA attributes
        for rva_tuple in rva_relation.tuples() {
            // Start with the cached non-RVA attributes
            let mut values = non_rva_values.clone();

            // Add RVA tuple's attribute values directly from its internal map
            for (attr_name, value) in rva_tuple.values() {
                values.insert(attr_name.clone(), value.clone());
            }

            let result_tuple = Tuple::new_unchecked(result_heading_arc.clone(), values);
            result_tuples.push(result_tuple);
        }
    }

    Ok(result_tuples)
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
