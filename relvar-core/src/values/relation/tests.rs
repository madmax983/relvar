#![allow(clippy::module_inception)]
use super::*;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{ScalarType, TupleType};

    fn emp_type() -> TupleType {
        TupleType::new()
            .with_attribute("emp_id", ScalarType::Int)
            .with_attribute("name", ScalarType::String)
    }

    #[test]
    fn test_relation_heading_is_tuple_type() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading.clone());
        let relation = Relation::new(rel_type);

        assert_eq!(relation.relation_type().heading(), &heading);
    }

    #[test]
    fn test_relation_body_is_set_no_duplicates() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        let tuple1 = tuple! { emp_id: 1i64, name: "Alice" };
        let tuple2 = tuple! { emp_id: 1i64, name: "Alice" }; // Duplicate

        assert!(relation.insert(tuple1).unwrap());
        assert!(!relation.insert(tuple2).unwrap()); // Returns false for duplicate

        assert_eq!(relation.cardinality(), 1);
    }

    #[test]
    fn test_relation_equality() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let mut rel1 = Relation::new(rel_type.clone());
        rel1.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();
        rel1.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap();

        let mut rel2 = Relation::new(rel_type);
        rel2.insert(tuple! { emp_id: 2i64, name: "Bob" }).unwrap(); // Different order
        rel2.insert(tuple! { emp_id: 1i64, name: "Alice" }).unwrap();

        assert_eq!(rel1, rel2);
    }

    #[test]
    fn test_cardinality_and_degree() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        assert_eq!(relation.cardinality(), 0);
        assert_eq!(relation.degree(), 2);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob" })
            .unwrap();

        assert_eq!(relation.cardinality(), 2);
        assert_eq!(relation.degree(), 2);
    }

    #[test]
    fn test_insert_type_mismatch() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        // Wrong type
        let wrong_tuple = tuple! { emp_id: 1i64, salary: 50000.0 };

        let result = relation.insert(wrong_tuple);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RelationError));
    }

    #[test]
    fn test_from_tuples() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);

        let tuples = vec![
            tuple! { emp_id: 1i64, name: "Alice" },
            tuple! { emp_id: 2i64, name: "Bob" },
            tuple! { emp_id: 1i64, name: "Alice" }, // Duplicate
        ];

        let relation = Relation::from_tuples(rel_type, tuples).unwrap();

        assert_eq!(relation.cardinality(), 2); // Duplicate removed
    }

    #[test]
    fn test_contains() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        let tuple1 = tuple! { emp_id: 1i64, name: "Alice" };
        let tuple2 = tuple! { emp_id: 2i64, name: "Bob" };

        relation.insert(tuple1.clone()).unwrap();

        assert!(relation.contains(&tuple1));
        assert!(!relation.contains(&tuple2));
    }

    #[test]
    fn should_create_relation_with_capacity() {
        let rel_type = RelationType::new(TupleType::new());
        let capacity = 100;
        let relation = Relation::with_capacity(rel_type, capacity);

        assert!(relation.is_empty());
        assert_eq!(relation.degree(), 0);
        assert!(relation.body.capacity() >= capacity);
    }

    #[test]
    fn test_empty_relation() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let relation = Relation::new(rel_type);

        assert!(relation.is_empty());
        assert_eq!(relation.cardinality(), 0);
    }

    #[test]
    fn test_tuples_iterator() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        relation
            .insert(tuple! { emp_id: 1i64, name: "Alice" })
            .unwrap();
        relation
            .insert(tuple! { emp_id: 2i64, name: "Bob" })
            .unwrap();

        let count = relation.tuples().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_from_tuples_optimization_correctness() {
        let heading = emp_type();
        let rel_type = RelationType::new(heading.clone());

        // Create a valid tuple
        let valid_tuple = tuple! { emp_id: 1i64, name: "Alice" };

        // Create an invalid tuple (wrong type)
        // Note: tuple! macro creates a tuple with inferred type from values.
        // So this tuple has a different TupleType than 'heading'.
        let invalid_tuple = tuple! { emp_id: 2i64, name: 12345 }; // Name is Int, expected String

        // Case 1: All valid (optimization path should work)
        let tuples = vec![valid_tuple.clone(), valid_tuple.clone()];
        let rel = Relation::from_tuples(rel_type.clone(), tuples);
        assert!(rel.is_ok());

        // Case 2: Mix valid and invalid (optimization should not hide error)
        // The first tuple sets the cached valid pointer.
        // The second tuple has a different pointer (and type), so it should be checked and fail.
        let tuples = vec![valid_tuple.clone(), invalid_tuple.clone()];
        let rel = Relation::from_tuples(rel_type.clone(), tuples);
        assert!(rel.is_err());
        assert!(matches!(rel.unwrap_err(), RelationError));

        // Case 3: Invalid first
        // The first tuple fails immediately.
        let tuples = vec![invalid_tuple.clone(), valid_tuple.clone()];
        let rel = Relation::from_tuples(rel_type.clone(), tuples);
        assert!(rel.is_err());
    }
}
