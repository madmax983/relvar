use crate::values::Relation;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IntersectError {
    #[error("Relations must have the same type (heading) for intersection")]
    TypeMismatch,
}

/// Intersection operation
impl Relation {
    /// Intersection with another relation
    /// Relations must be type-compatible (same heading)
    /// Result contains only tuples that appear in both relations
    pub fn intersect(&self, other: &Relation) -> Result<Self, IntersectError> {
        // Check type compatibility
        if self.relation_type() != other.relation_type() {
            return Err(IntersectError::TypeMismatch);
        }

        // Find tuples that exist in both relations
        let common_tuples: Vec<_> = self
            .tuples()
            .filter(|tuple| other.contains(tuple))
            .cloned()
            .collect();

        Ok(
            Relation::from_tuples(self.relation_type().clone(), common_tuples)
                .expect("Intersection tuples should conform to relation type"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::values::Relation;

    fn emp_type() -> TupleType {
        TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
    }

    #[test]
    fn test_intersect_returns_common_tuples() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
        rel1.insert(tuple! { emp_id: 3i64, name: "Charlie" })
            .unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();
        rel2.insert(tuple! { emp_id: 3i64, name: "Charlie" })
            .unwrap();
        rel2.insert(tuple! { emp_id: 4i64, name: "David" }).unwrap();

        let result = rel1.intersect(&rel2).unwrap();

        assert_eq!(result.cardinality(), 2); // Bob and Charlie

        let bob = tuple! { emp_id: 2i64, name: "Bob" };
        let charlie = tuple! { emp_id: 3i64, name: "Charlie" };

        assert!(result.contains(&bob));
        assert!(result.contains(&charlie));
    }

    #[test]
    fn test_intersect_type_mismatch() {
        let type1 = emp_type();
        let rel_type1 = RelationType::new(type1);
        let rel1 = Relation::new(rel_type1);

        let type2 = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("salary", ScalarType::Float);

        let rel_type2 = RelationType::new(type2);
        let rel2 = Relation::new(rel_type2);

        let result = rel1.intersect(&rel2);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), IntersectError::TypeMismatch));
    }

    #[test]
    fn test_intersect_with_empty_relation() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let rel2 = Relation::new(rel_type);

        let result = rel1.intersect(&rel2).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_intersect_no_common_tuples() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let result = rel1.intersect(&rel2).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_intersect_identical_relations() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type);
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let result = rel1.intersect(&rel1).unwrap();
        assert_eq!(result, rel1);
    }
}
