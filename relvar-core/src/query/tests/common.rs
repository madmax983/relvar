use crate::database::Database;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue};

pub(crate) fn setup_db() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());

    let users_heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("age", ScalarType::Int);

    db.create_relvar("USERS", RelationType::new(users_heading.clone()))
        .unwrap();

    let users_rel_type = RelationType::new(users_heading.clone());
    let mut users_rel = Relation::new(users_rel_type);
    users_rel
        .insert(tuple![
            id: ScalarValue::Int(1),
            name: ScalarValue::String("Alice".into()),
            age: ScalarValue::Int(30)
        ])
        .unwrap();
    users_rel
        .insert(tuple![
            id: ScalarValue::Int(2),
            name: ScalarValue::String("Bob".into()),
            age: ScalarValue::Int(25)
        ])
        .unwrap();
    for t in users_rel.tuples() {
        db.insert("USERS", t.clone()).unwrap();
    }

    let orders_heading = TupleType::new()
        .with_attribute("order_id", ScalarType::Int)
        .with_attribute("id", ScalarType::Int) // FK to users
        .with_attribute("amount", ScalarType::Int);

    let orders_rel_type = RelationType::new(orders_heading.clone());
    db.create_relvar("ORDERS", orders_rel_type.clone()).unwrap();
    let mut orders_rel = Relation::new(orders_rel_type);
    orders_rel
        .insert(tuple![
            order_id: ScalarValue::Int(101),
            id: ScalarValue::Int(1),
            amount: ScalarValue::Int(500)
        ])
        .unwrap();
    for t in orders_rel.tuples() {
        db.insert("ORDERS", t.clone()).unwrap();
    }

    db
}
