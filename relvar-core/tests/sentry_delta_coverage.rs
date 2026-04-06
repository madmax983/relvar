use relvar_core::algebra::delta::Delta;
use relvar_core::error::DatabaseError;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;

#[test]
fn should_return_error_when_type_mismatch() {
    let heading1 = TupleType::new().with_attribute("id", ScalarType::Int);
    let rel_type1 = RelationType::new(heading1);

    let heading2 = TupleType::new().with_attribute("id", ScalarType::String);
    let rel_type2 = RelationType::new(heading2);

    let inserted = Relation::new(rel_type1);
    let deleted = Relation::new(rel_type2);

    let delta = Delta::new(inserted, deleted);
    assert!(delta.is_err());

    if let Err(DatabaseError::AlgebraError(msg)) = delta {
        assert!(
            msg.contains("Type mismatch: Inserted and deleted relations must have the same type")
        );
    } else {
        panic!("Expected DatabaseError::AlgebraError");
    }
}
