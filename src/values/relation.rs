use crate::types::RelationType;
use crate::values::Tuple;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RelationError {
    #[error("Tuple does not conform to relation type")]
    TypeMismatch,
    #[error("Duplicate tuple")]
    DuplicateTuple,
}

/// A relation value: a heading (relation type) and a body (set of tuples).
/// Per Date's relational model:
/// - The body is a true set (no duplicates)
/// - All tuples conform to the heading
/// - Relations are equal iff they have the same heading and same body
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    relation_type: RelationType,
    body: HashSet<Tuple>,
}

// Custom Hash implementation for Relation
// Since HashSet doesn't have a deterministic iteration order, we need to
// hash the relation in a way that's independent of the set's internal ordering
impl std::hash::Hash for Relation {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the relation type
        self.relation_type.hash(state);

        // Hash the cardinality
        self.body.len().hash(state);

        // Hash all tuples in a deterministic way by XORing their hashes
        // This works because XOR is commutative and associative
        let mut combined_hash = 0u64;
        for tuple in &self.body {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            tuple.hash(&mut hasher);
            combined_hash ^= std::hash::Hasher::finish(&hasher);
        }
        combined_hash.hash(state);
    }
}

impl Relation {
    /// Create a new empty relation with the given type
    pub fn new(relation_type: RelationType) -> Self {
        Self {
            relation_type,
            body: HashSet::new(),
        }
    }

    /// Create a relation from a type and a set of tuples
    pub fn from_tuples(
        relation_type: RelationType,
        tuples: impl IntoIterator<Item = Tuple>,
    ) -> Result<Self, RelationError> {
        let mut body = HashSet::new();

        for tuple in tuples {
            // Verify tuple conforms to the relation type
            if tuple.tuple_type() != relation_type.heading() {
                return Err(RelationError::TypeMismatch);
            }
            body.insert(tuple);
        }

        Ok(Self {
            relation_type,
            body,
        })
    }

    /// Get the relation type
    pub fn relation_type(&self) -> &RelationType {
        &self.relation_type
    }

    /// Get the cardinality (number of tuples)
    pub fn cardinality(&self) -> usize {
        self.body.len()
    }

    /// Get the degree (number of attributes)
    pub fn degree(&self) -> usize {
        self.relation_type.degree()
    }

    /// Insert a tuple into the relation
    /// Returns Ok(true) if inserted, Ok(false) if duplicate
    pub fn insert(&mut self, tuple: Tuple) -> Result<bool, RelationError> {
        // Verify tuple conforms to the relation type
        if tuple.tuple_type() != self.relation_type.heading() {
            return Err(RelationError::TypeMismatch);
        }

        Ok(self.body.insert(tuple))
    }

    /// Check if the relation contains a tuple
    pub fn contains(&self, tuple: &Tuple) -> bool {
        self.body.contains(tuple)
    }

    /// Get an iterator over the tuples
    pub fn tuples(&self) -> impl Iterator<Item = &Tuple> {
        self.body.iter()
    }

    /// Check if the relation is empty
    pub fn is_empty(&self) -> bool {
        self.body.is_empty()
    }
}

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
        assert!(matches!(result.unwrap_err(), RelationError::TypeMismatch));
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
}
