use std::collections::BTreeMap;
use std::sync::Arc;

pub fn compute_ungrouped_tuples(
    relation: &Relation,
    rva_name: &str,
    result_heading: &TupleType,
    rva_relation_type: &RelationType,
) -> Result<Vec<Tuple>, UngroupError> {
    let mut result_tuples = Vec::new();
    let result_heading_arc = Arc::new(result_heading.clone());

    for tuple in relation.tuples() {
        // Get the RVA relation
        let rva_relation = match tuple.get(rva_name).unwrap() {
            ScalarValue::Relation(rel) => rel,
            _ => return Err(UngroupError::NotRelationValued(rva_name.to_string())),
        };

        // Cache the non-RVA attribute values for this tuple
        // Instead of re-extracting and cloning them for every tuple in the RVA
        let mut non_rva_values = BTreeMap::new();
        for (attr_name, value) in tuple.values() {
            if attr_name != rva_name {
                non_rva_values.insert(attr_name.clone(), value.clone());
            }
        }

        // For each tuple in the RVA, create a new tuple combining non-RVA and RVA attributes
        for rva_tuple in rva_relation.tuples() {
            // Clone the cached non-RVA values
            let mut values = non_rva_values.clone();

            // Add RVA tuple's attribute values (can just extend from its internal values directly)
            // Need to handle potential cloning here but it's simpler
            for (attr_name, value) in rva_tuple.values() {
                values.insert(attr_name.clone(), value.clone());
            }

            let result_tuple = Tuple::new_unchecked(result_heading_arc.clone(), values);
            result_tuples.push(result_tuple);
        }
    }

    Ok(result_tuples)
}
