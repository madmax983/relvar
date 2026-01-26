use crate::values::Relation;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UnionError {
    #[error("Relations must have the same type (heading) for union")]
    TypeMismatch,
}

/// Union operation
impl Relation {
    /// Union with another relation
    /// Relations must be type-compatible (same heading)
    /// Result contains all tuples from both relations, with duplicates removed
    pub fn union(&self, other: &Relation) -> Result<Self, UnionError> {
        // Check type compatibility
        if self.relation_type() != other.relation_type() {
            return Err(UnionError::TypeMismatch);
        }

        // Collect all tuples from both relations
        let mut all_tuples: Vec<_> = self.tuples().cloned().collect();
        all_tuples.extend(other.tuples().cloned());

        // from_tuples automatically removes duplicates via HashSet
        Ok(
            Relation::from_tuples(self.relation_type().clone(), all_tuples)
                .expect("Union tuples should conform to relation type"),
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
    fn test_union_requires_type_compatible_relations() {
        let type1 = emp_type();
        let rel_type1 = RelationType::new(type1);
        let rel1 = Relation::new(rel_type1);

        let type2 = TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("salary", ScalarType::Float);

        let rel_type2 = RelationType::new(type2);
        let rel2 = Relation::new(rel_type2);

        let result = rel1.union(&rel2);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), UnionError::TypeMismatch));
    }

    #[test]
    fn test_union_removes_duplicates() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap(); // Duplicate
        rel2.insert(tuple! { emp_id: 3i64, name: "Charlie" })
            .unwrap();

        let result = rel1.union(&rel2).unwrap();

        assert_eq!(result.cardinality(), 3); // Alice, Bob, Charlie (Bob not duplicated)

        let alice = tuple! { emp_id: 1i64, name: "Alice" };
        let bob = tuple! { emp_id: 2i64, name: "Bob" };
        let charlie = tuple! { emp_id: 3i64, name: "Charlie" };

        assert!(result.contains(&alice));
        assert!(result.contains(&bob));
        assert!(result.contains(&charlie));
    }

    #[test]
    fn test_union_with_empty_relation() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let rel2 = Relation::new(rel_type);

        let result = rel1.union(&rel2).unwrap();
        assert_eq!(result.cardinality(), 1);
        assert_eq!(result, rel1);
    }

    #[test]
    fn test_union_of_disjoint_relations() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let result = rel1.union(&rel2).unwrap();
        assert_eq!(result.cardinality(), 2);
    }

    #[test]
    fn test_union_identical_relations() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let result = rel1.union(&rel1).unwrap();
        assert_eq!(result.cardinality(), 2); // No duplicates
        assert_eq!(result, rel1);
    }
}
