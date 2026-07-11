#![allow(clippy::module_inception)]
use super::*;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::{
        AttributeConstraints, CandidateKey, CheckConstraint, ConstraintExpression, PrimaryKey,
        TypeConstraint,
    };
    use crate::storage_engine::InMemoryEngine;
    use crate::tuple;
    use crate::types::{RelationType, ScalarType, TupleType};
    use crate::{CmpOp, Relation, ScalarValue, ValueOrRef};

    fn setup_engine() -> InMemoryEngine {
        let mut engine = InMemoryEngine::new();

        let t1 = RelationType::new(TupleType::new().with_attribute("id", ScalarType::Int));
        engine.create_relation("users", t1).unwrap();

        let mut users = Relation::new(RelationType::new(
            TupleType::new().with_attribute("id", ScalarType::Int),
        ));
        users.insert(tuple! { id: 1i64 }).unwrap();
        engine.store_relation("users", &users).unwrap();

        engine
    }

    #[test]
    fn should_return_error_when_check_constraint_violates_existing_data() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let check_expr = ConstraintExpression::Cmp {
            left: "id".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(2)), // current data has id:1
        };
        let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "id_is_2".to_string(),
            "must be 2".to_string(),
            check_expr,
        ));

        let res = manager.set_check_constraints(&mut engine, "users", checks);
        assert!(res.is_err());
    }

    #[test]
    fn should_allow_tuple_type_validation_without_constraints() {
        let engine = setup_engine();
        let manager = ConstraintManager::new();

        let good_tuple = tuple! { id: 1i64 };
        let bad_tuple = tuple! { id: "wrong_type".to_string() };

        assert!(
            manager
                .validate_type_constraints("users", &bad_tuple)
                .is_ok()
        );
        assert!(
            manager
                .validate_tuple_type(&engine, "users", &good_tuple)
                .is_ok()
        );
        assert!(
            manager
                .validate_tuple_type(&engine, "users", &bad_tuple)
                .is_err()
        );
    }

    #[test]
    fn should_return_error_when_type_constraint_violated() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let mut attr_cons = AttributeConstraints::new("id".to_string(), ScalarType::Int);
        let type_constraint = TypeConstraint::Range {
            min: ScalarValue::Int(1),
            max: ScalarValue::Int(1),
        };
        attr_cons = attr_cons.with_constraint(type_constraint);
        manager
            .set_type_constraints(&mut engine, "users", "id", attr_cons)
            .unwrap();

        let res = manager.validate_type_constraints("users", &tuple! { id: 2i64 });
        assert!(res.is_err());

        let res = manager.validate_check_constraints("users", &tuple! { id: 1i64 });
        assert!(res.is_ok());
    }

    #[test]
    fn should_return_error_when_key_constraints_violated_in_single_tuple() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let ck = CandidateKey::new(vec!["id".to_string()]).unwrap();
        let keys = KeyConstraints::new()
            .with_primary_key(pk)
            .with_candidate_key(ck);

        manager
            .set_key_constraints(&mut engine, "users", keys.clone())
            .unwrap();

        let rel = engine.load_relation("users").unwrap();
        // single tuple pk violation (1i64 already exists)
        assert!(
            manager
                .validate_key_constraints_single_tuple("users", &tuple! { id: 1i64 }, &rel)
                .is_err()
        );
    }

    #[test]
    fn should_return_error_when_candidate_key_violated_in_single_tuple() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let bad_keys = KeyConstraints::new()
            .with_candidate_key(CandidateKey::new(vec!["id".to_string()]).unwrap());
        manager
            .set_key_constraints(&mut engine, "users", bad_keys)
            .unwrap();

        let rel = engine.load_relation("users").unwrap();
        assert!(
            manager
                .validate_key_constraints_single_tuple("users", &tuple! { id: 1i64 }, &rel)
                .is_err()
        );
    }

    #[test]
    fn should_return_error_when_bulk_key_constraints_violated() {
        let manager = ConstraintManager::new();
        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let ck = CandidateKey::new(vec!["id".to_string()]).unwrap();
        let keys = KeyConstraints::new()
            .with_primary_key(pk)
            .with_candidate_key(ck);

        let t3 = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("val", ScalarType::Int),
        );
        let mut dup_rel = Relation::new(t3);
        dup_rel.insert(tuple! { id: 1i64, val: 1i64 }).unwrap();
        dup_rel.insert(tuple! { id: 1i64, val: 2i64 }).unwrap();

        assert!(
            manager
                .validate_key_constraints_bulk(&dup_rel, &keys)
                .is_err()
        );
    }

    #[test]
    fn should_return_correct_constraints_when_queried_and_removed() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let pk = PrimaryKey::new(vec!["id".to_string()]).unwrap();
        let ck = CandidateKey::new(vec!["id".to_string()]).unwrap();
        let keys = KeyConstraints::new()
            .with_primary_key(pk)
            .with_candidate_key(ck);

        manager
            .set_key_constraints(&mut engine, "users", keys.clone())
            .unwrap();

        assert!(manager.get_key_constraints("users").is_some());
        assert!(manager.get_foreign_key_constraints("users").is_none());

        manager.remove_constraints_for_relation("users");
        assert!(manager.get_key_constraints("users").is_none());
    }

    fn setup_engine_with_posts() -> InMemoryEngine {
        let mut engine = setup_engine();

        let t_posts = RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("user_id", ScalarType::Int),
        );
        engine.create_relation("posts", t_posts).unwrap();

        let mut posts = Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("user_id", ScalarType::Int),
        ));
        posts.insert(tuple! { id: 100i64, user_id: 1i64 }).unwrap();
        engine.store_relation("posts", &posts).unwrap();

        engine
    }

    #[test]
    fn should_return_error_when_setting_foreign_keys_on_nonexistent_relation() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let fk_constraints = ForeignKeyConstraints::new();

        let res = manager.set_foreign_key_constraints(&mut engine, "missing", fk_constraints);
        assert!(matches!(
            res,
            Err(ConstraintManagerError::RelationNotFound(_))
        ));
    }

    #[test]
    fn should_set_foreign_keys_when_data_is_valid() {
        let mut engine = setup_engine_with_posts();
        let mut manager = ConstraintManager::new();

        let fk = crate::constraints::ForeignKey::new(
            vec!["user_id".to_string()],
            "users".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();

        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

        let res = manager.set_foreign_key_constraints(&mut engine, "posts", fk_constraints);
        assert!(res.is_ok());
    }

    #[test]
    fn should_return_error_when_setting_foreign_keys_violates_existing_data() {
        let mut engine = setup_engine_with_posts();
        let mut manager = ConstraintManager::new();

        // Insert a post with a non-existent user_id
        let mut posts = engine.load_relation("posts").unwrap();
        posts.insert(tuple! { id: 101i64, user_id: 99i64 }).unwrap();
        engine.store_relation("posts", &posts).unwrap();

        let fk = crate::constraints::ForeignKey::new(
            vec!["user_id".to_string()],
            "users".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();

        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);

        let res = manager.set_foreign_key_constraints(&mut engine, "posts", fk_constraints);
        assert!(matches!(
            res,
            Err(ConstraintManagerError::ForeignKeyViolation(_))
        ));
    }

    #[test]
    fn should_validate_foreign_keys_for_single_tuple() {
        let mut engine = setup_engine_with_posts();
        let mut manager = ConstraintManager::new();

        let fk = crate::constraints::ForeignKey::new(
            vec!["user_id".to_string()],
            "users".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();

        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
        manager
            .set_foreign_key_constraints(&mut engine, "posts", fk_constraints)
            .unwrap();

        // Valid tuple
        let valid_tuple = tuple! { id: 102i64, user_id: 1i64 };
        let res = manager.validate_foreign_keys_single_tuple(&mut engine, "posts", &valid_tuple);
        assert!(res.is_ok());

        // Invalid tuple (violates foreign key)
        let invalid_tuple = tuple! { id: 103i64, user_id: 99i64 };
        let res = manager.validate_foreign_keys_single_tuple(&mut engine, "posts", &invalid_tuple);
        assert!(matches!(
            res,
            Err(ConstraintManagerError::ForeignKeyViolation(_))
        ));
    }

    #[test]
    fn should_validate_referencing_foreign_keys_on_delete() {
        let mut engine = setup_engine_with_posts();
        let mut manager = ConstraintManager::new();

        let fk = crate::constraints::ForeignKey::new(
            vec!["user_id".to_string()],
            "users".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();

        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
        manager
            .set_foreign_key_constraints(&mut engine, "posts", fk_constraints)
            .unwrap();

        // Simulate deleting the user with id=1 from users
        let empty_users = Relation::new(RelationType::new(
            TupleType::new().with_attribute("id", ScalarType::Int),
        ));

        // This should fail because `posts` has a record referencing `user_id = 1`
        let res = manager.validate_referencing_foreign_keys(&mut engine, "users", &empty_users);
        assert!(matches!(
            res,
            Err(ConstraintManagerError::ForeignKeyViolation(_))
        ));

        // Empty out posts so no references remain
        let empty_posts = Relation::new(RelationType::new(
            TupleType::new()
                .with_attribute("id", ScalarType::Int)
                .with_attribute("user_id", ScalarType::Int),
        ));
        engine.store_relation("posts", &empty_posts).unwrap();

        // Now validating deletion of users should pass
        let res = manager.validate_referencing_foreign_keys(&mut engine, "users", &empty_users);
        assert!(res.is_ok());
    }

    #[test]
    fn should_validate_tuple_content_constraints() {
        let mut engine = setup_engine_with_posts();
        let mut manager = ConstraintManager::new();

        // 1. Add Type Constraint on posts.id (min: 10)
        let type_cons = TypeConstraint::Range {
            min: ScalarValue::Int(10),
            max: ScalarValue::Int(1000),
        };
        manager
            .set_type_constraints(
                &mut engine,
                "posts",
                "id",
                AttributeConstraints::new("id".to_string(), ScalarType::Int)
                    .with_constraint(type_cons),
            )
            .unwrap();

        // 2. Add CHECK Constraint on posts.user_id (must be > 0)
        let check_expr = ConstraintExpression::Cmp {
            left: "user_id".to_string(),
            op: CmpOp::Gt,
            right: ValueOrRef::Value(ScalarValue::Int(0)),
        };
        let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "user_id_positive".to_string(),
            "must be positive".to_string(),
            check_expr,
        ));
        manager
            .set_check_constraints(&mut engine, "posts", checks)
            .unwrap();

        // 3. Add Foreign Key Constraint on posts.user_id -> users.id
        let fk = crate::constraints::ForeignKey::new(
            vec!["user_id".to_string()],
            "users".to_string(),
            vec!["id".to_string()],
        )
        .unwrap();
        let fk_constraints = ForeignKeyConstraints::new().with_foreign_key(fk);
        manager
            .set_foreign_key_constraints(&mut engine, "posts", fk_constraints)
            .unwrap();

        // Test Type violation (id < 10)
        let bad_type_tuple = tuple! { id: 5i64, user_id: 1i64 };
        assert!(matches!(
            manager.validate_tuple_content_constraints(&mut engine, "posts", &bad_type_tuple),
            Err(ConstraintManagerError::TypeConstraintViolation(_))
        ));

        // Test CHECK violation (user_id <= 0)
        let bad_check_tuple = tuple! { id: 100i64, user_id: 0i64 };
        assert!(matches!(
            manager.validate_tuple_content_constraints(&mut engine, "posts", &bad_check_tuple),
            Err(ConstraintManagerError::CheckConstraintViolation(_))
        ));

        // Test Foreign Key violation (user_id does not exist in users)
        let bad_fk_tuple = tuple! { id: 100i64, user_id: 999i64 };
        assert!(matches!(
            manager.validate_tuple_content_constraints(&mut engine, "posts", &bad_fk_tuple),
            Err(ConstraintManagerError::ForeignKeyViolation(_))
        ));

        // Test valid tuple
        let valid_tuple = tuple! { id: 150i64, user_id: 1i64 };
        assert!(
            manager
                .validate_tuple_content_constraints(&mut engine, "posts", &valid_tuple)
                .is_ok()
        );
    }

    #[test]
    fn should_return_error_when_setting_check_constraints_on_nonexistent_relation() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let check_expr = ConstraintExpression::Cmp {
            left: "id".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(2)),
        };
        let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "test_check".to_string(),
            "test".to_string(),
            check_expr,
        ));

        let res = manager.set_check_constraints(&mut engine, "missing", checks);
        assert!(matches!(
            res,
            Err(ConstraintManagerError::RelationNotFound(_))
        ));
    }

    #[test]
    fn should_return_error_when_setting_check_constraints_on_nonexistent_attribute() {
        let mut engine = setup_engine();
        let mut manager = ConstraintManager::new();

        let check_expr = ConstraintExpression::Cmp {
            left: "missing_attr".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(2)),
        };
        let checks = CheckConstraints::new().with_constraint(CheckConstraint::new(
            "test_check".to_string(),
            "test".to_string(),
            check_expr,
        ));

        let res = manager.set_check_constraints(&mut engine, "users", checks);
        assert!(
            matches!(res, Err(ConstraintManagerError::AttributeNotFound(attr, _)) if attr == "missing_attr")
        );
    }
}
