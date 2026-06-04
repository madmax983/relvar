use relvar_core::{Database, RelationType, ScalarType, TupleType, tuple};
use relvar_core::storage_engine::InMemoryEngine;

#[test]
fn test_gc() {
    let mut db = Database::new(InMemoryEngine::new());

    let root_type = RelationType::new(
        TupleType::new().with_attribute("id", ScalarType::Int)
    );
    let heap_type = RelationType::new(
        TupleType::new().with_attribute("id", ScalarType::Int)
    );
    let ref_type = RelationType::new(
        TupleType::new()
            .with_attribute("from_id", ScalarType::Int)
            .with_attribute("to_id", ScalarType::Int)
    );

    db.create_relvar("roots", root_type).unwrap();
    db.create_relvar("heap", heap_type).unwrap();
    db.create_relvar("references", ref_type).unwrap();

    // Heap objects 1, 2, 3, 4, 5
    for id in 1..=5 {
        db.insert("heap", tuple! { id: id as i64 }).unwrap();
    }

    // Object 1 is a root
    db.insert("roots", tuple! { id: 1i64 }).unwrap();

    // 1 -> 2
    // 2 -> 3
    // 4 -> 5
    db.insert("references", tuple! { from_id: 1i64, to_id: 2i64 }).unwrap();
    db.insert("references", tuple! { from_id: 2i64, to_id: 3i64 }).unwrap();
    db.insert("references", tuple! { from_id: 4i64, to_id: 5i64 }).unwrap();

    let roots = db.query("roots").unwrap();
    let references = db.query("references").unwrap();

    let closure = references.tclose("from_id", "to_id").unwrap();
    let roots_as_from = roots.clone().rename(&[("id", "from_id")]);
    let reachable_edges = roots_as_from.join(&closure).unwrap();
    let reached_to_ids = reachable_edges.project(&["to_id"]);
    let reached_ids = reached_to_ids.rename(&[("to_id", "id")]);
    let marked = roots.union(&reached_ids).unwrap();

    let heap = db.query("heap").unwrap();
    let garbage = heap.difference(&marked).unwrap();

    assert_eq!(garbage.cardinality(), 2); // 4 and 5
}
