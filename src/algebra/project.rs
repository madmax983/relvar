use crate::types::{RelationType, TupleType};
use crate::values::{Relation, Tuple};
use std::collections::BTreeMap;

/// Project operation (SELECT columns in SQL)
/// Selects a subset of attributes
impl Relation {
    /// Project this relation onto a subset of attributes
    pub fn project(&self, attributes: &[&str]) -> Self {
        // Build new heading with selected attributes
        let mut new_heading = TupleType::new();
        for attr_name in attributes {
            if let Some(attr_type) = self.relation_type().heading().get_attribute_type(attr_name) {
                new_heading = new_heading.with_attribute(*attr_name, attr_type.clone());
            }
        }

        let new_rel_type = RelationType::new(new_heading.clone());

        // Project each tuple
        let projected_tuples: Vec<_> = self
            .tuples()
            .map(|tuple| {
                let mut values = BTreeMap::new();
                for attr_name in attributes {
                    if let Some(value) = tuple.get(attr_name) {
                        values.insert(attr_name.to_string(), value.clone());
                    }
                }
                Tuple::new(new_heading.clone(), values)
                    .expect("Projection should maintain type consistency")
            })
            .collect();

        // Duplicates are automatically removed when creating the relation
        Relation::from_tuples(new_rel_type, projected_tuples)
            .expect("Projected tuples should conform to new relation type")
    }
}

#[cfg(test)]
mod tests {
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    #[test]
    fn test_project_selects_attributes() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        let result = relation.project(&["emp_id", "name"]);

        assert_eq!(result.degree(), 2);
        assert_eq!(result.cardinality(), 2);

        // Verify result has correct attributes
        let alice_projected = tuple! { emp_id: 1i64, name: "Alice" };
        assert!(result.contains(&alice_projected));
    }

    #[test]
    fn test_project_removes_duplicates() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 3i64, name: "Charlie", dept_id: 10i64 })
            .unwrap();

        // Project onto just dept_id - should have only one unique value
        let result = relation.project(&["dept_id"]);

        assert_eq!(result.degree(), 1);
        assert_eq!(result.cardinality(), 1); // All have same dept_id, duplicates removed
    }

    #[test]
    fn test_project_empty_relation() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        let result = relation.project(&["emp_id"]);

        assert_eq!(result.cardinality(), 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_project_all_attributes() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob" })
            .unwrap();

        let result = relation.project(&["emp_id", "name"]);

        assert_eq!(result, relation);
    }

    #[test]
    fn test_project_single_attribute() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob", dept_id: 20i64 })
            .unwrap();

        let result = relation.project(&["name"]);

        assert_eq!(result.degree(), 1);
        assert_eq!(result.cardinality(), 2);
    }
}
