use crate::types::{RelationType, TupleType};
use crate::values::{Relation, Tuple};
use std::collections::BTreeMap;

/// Rename operation
/// Renames attributes while preserving types
impl Relation {
    /// Rename attributes according to the provided mapping
    /// Maps old_name -> new_name
    pub fn rename(&self, mappings: &[(&str, &str)]) -> Self {
        // Build new heading with renamed attributes
        let mut new_heading = TupleType::new();

        for (old_name, attr_type) in self.relation_type().heading().attributes() {
            // Check if this attribute should be renamed
            let new_name = mappings
                .iter()
                .find(|(from, _)| from == old_name)
                .map(|(_, to)| *to)
                .unwrap_or(old_name.as_str());

            new_heading = new_heading.with_attribute(new_name, attr_type.clone());
        }

        let new_rel_type = RelationType::new(new_heading.clone());

        // Rename attributes in each tuple
        let renamed_tuples: Vec<_> = self
            .tuples()
            .map(|tuple| {
                let mut values = BTreeMap::new();

                for (old_name, value) in tuple.values() {
                    let new_name = mappings
                        .iter()
                        .find(|(from, _)| from == old_name)
                        .map(|(_, to)| *to)
                        .unwrap_or(old_name.as_str());

                    values.insert(new_name.to_string(), value.clone());
                }

                Tuple::new(new_heading.clone(), values)
                    .expect("Rename should maintain type consistency")
            })
            .collect();

        Relation::from_tuples(new_rel_type, renamed_tuples)
            .expect("Renamed tuples should conform to new relation type")
    }
}

#[cfg(test)]
mod tests {
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    #[test]
    fn test_rename_changes_attribute_name() {
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

        let result = relation.rename(&[("emp_id", "id")]);

        // Check that new attribute exists and old doesn't
        assert!(result.relation_type().heading().has_attribute("id"));
        assert!(!result.relation_type().heading().has_attribute("emp_id"));
        assert!(result.relation_type().heading().has_attribute("name"));

        // Check cardinality preserved
        assert_eq!(result.cardinality(), 2);
    }

    #[test]
    fn test_rename_preserves_type() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();

        let result = relation.rename(&[("emp_id", "employee_id")]);

        // Verify type preserved
        assert_eq!(
            result
                .relation_type()
                .heading()
                .get_attribute_type("employee_id"),
            Some(&ScalarType::Int)
        );
    }

    #[test]
    fn test_rename_multiple_attributes() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
            .with_attribute("dept_id", ScalarType::Int);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice", dept_id: 10i64 })
            .unwrap();

        let result = relation.rename(&[("emp_id", "employee_id"), ("dept_id", "department_id")]);

        assert!(
            result
                .relation_type()
                .heading()
                .has_attribute("employee_id")
        );
        assert!(
            result
                .relation_type()
                .heading()
                .has_attribute("department_id")
        );
        assert!(result.relation_type().heading().has_attribute("name"));
        assert!(!result.relation_type().heading().has_attribute("emp_id"));
        assert!(!result.relation_type().heading().has_attribute("dept_id"));
    }

    #[test]
    fn test_rename_empty_mappings() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading.clone());
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();

        let result = relation.rename(&[]);

        // Should be unchanged
        assert_eq!(result.relation_type().heading(), &heading);
        assert_eq!(result.cardinality(), 1);
    }

    #[test]
    fn test_rename_on_empty_relation() {
        let heading = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String);

        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        let result = relation.rename(&[("emp_id", "id")]);

        assert!(result.is_empty());
        assert!(result.relation_type().heading().has_attribute("id"));
    }
}
