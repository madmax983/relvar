use crate::database::Database;
use crate::storage_engine::InMemoryEngine;
use crate::tuple;
use crate::types::{RelationType, ScalarType, TupleType};
use crate::values::{Relation, ScalarValue};

fn setup_users(db: &mut Database<InMemoryEngine>) {
    let users_heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("age", ScalarType::Int);

    db.create_relvar("USERS", RelationType::new(users_heading)).unwrap();

    db.insert("USERS", tuple![
        id: ScalarValue::Int(1),
        name: ScalarValue::String("Alice".into()),
        age: ScalarValue::Int(30)
    ]).unwrap();
    db.insert("USERS", tuple![
        id: ScalarValue::Int(2),
        name: ScalarValue::String("Bob".into()),
        age: ScalarValue::Int(25)
    ]).unwrap();
}

fn setup_orders(db: &mut Database<InMemoryEngine>) {
    let orders_heading = TupleType::new()
        .with_attribute("order_id", ScalarType::Int)
        .with_attribute("id", ScalarType::Int) // FK to users
        .with_attribute("amount", ScalarType::Int);

    db.create_relvar("ORDERS", RelationType::new(orders_heading)).unwrap();

    db.insert("ORDERS", tuple![
        order_id: ScalarValue::Int(101),
        id: ScalarValue::Int(1),
        amount: ScalarValue::Int(500)
    ]).unwrap();
}

pub fn setup_db() -> Database<InMemoryEngine> {
    let mut db = Database::new(InMemoryEngine::new());
    setup_users(&mut db);
    setup_orders(&mut db);
    db
}
