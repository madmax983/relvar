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

    new_code = """fn compute_ungrouped_tuples<'a>(
    relation: &'a Relation,
    rva_name: &'a str,
    result_heading: std::sync::Arc<TupleType>,
    _rva_relation_type: &'a RelationType,
) -> Result<impl Iterator<Item = Tuple> + 'a, UngroupError> {
    // Validate first since we return an iterator
    for tuple in relation.tuples() {
        if let ScalarValue::Relation(_) = tuple.get(rva_name).unwrap() {
        } else {
            return Err(UngroupError::NotRelationValued(rva_name.to_string()));
        }
    }

    let iter = relation.tuples().flat_map(move |tuple| {
        // Extract the RVA relation from the tuple
        let rva_relation = match tuple.get(rva_name).unwrap() {
            ScalarValue::Relation(rel) => rel,
            _ => unreachable!(), // Validated above
        };

        // Cache the non-RVA attributes to avoid extracting and allocating them
        // inside the RVA loop. We know the exact capacity needed.
        let mut non_rva_values = Vec::with_capacity(tuple.degree() - 1);
        for (attr_name, value) in tuple.values() {
            if attr_name != rva_name {
                non_rva_values.push((attr_name, value));
            }
        }

        let heading_arc = result_heading.clone();

        // Map over the RVA tuples and combine them with the non-RVA attributes
        rva_relation.tuples().map(move |rva_tuple| {
            let mut combined_values = std::collections::BTreeMap::new();

            // Insert the non-RVA values
            for &(attr_name, value) in &non_rva_values {
                combined_values.insert(attr_name.clone(), value.clone());
            }

            for (attr_name, value) in rva_tuple.values() {
                combined_values.insert(attr_name.clone(), value.clone());
            }

            Tuple::new_unchecked(heading_arc.clone(), combined_values)
        }).collect::<Vec<_>>().into_iter()
    });

    Ok(iter)
}"""

    if old_code in content:
        content = content.replace(old_code, new_code)

        # update the parent ungroup func call
        parent_old = """        // 3. Compute ungrouped tuples
        let result_tuples =
            compute_ungrouped_tuples(self, rva_name, &result_heading, &rva_relation_type)?;

        Ok(
            Relation::from_tuples(RelationType::new(result_heading), result_tuples)
                .expect("Ungrouped tuples should conform to result relation type"),
        )"""

        parent_new = """        let result_heading_arc = std::sync::Arc::new(result_heading.clone());

        // 3. Compute ungrouped tuples via iterator (bypasses intermediate Vec allocation and validation)
        let result_tuple_iter =
            compute_ungrouped_tuples(self, rva_name, result_heading_arc, &rva_relation_type)?;

        Ok(Relation::from_tuples_unchecked(
            RelationType::new(result_heading),
            result_tuple_iter,
        ))"""

        content = content.replace(parent_old, parent_new)

        with open('relvar-core/src/algebra/group.rs', 'w') as f:
            f.write(content)
        print("Success")
    else:
        print("Could not find code to replace")

if __name__ == "__main__":
    main()
