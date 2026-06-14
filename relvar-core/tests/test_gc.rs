use relvar_core::experimental::garbage_collector::sweep_garbage;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;

#[test]
fn test_garbage_collector() {
    let obj_type = RelationType::new(
        TupleType::new()
            .with_attribute("object_id", ScalarType::Int)
            .with_attribute("size", ScalarType::Int),
    );
    let mut objects = Relation::new(obj_type);
    objects
        .insert(tuple! { object_id: 1i64, size: 10i64 })
        .unwrap();
    objects
        .insert(tuple! { object_id: 2i64, size: 20i64 })
        .unwrap();
    objects
        .insert(tuple! { object_id: 3i64, size: 30i64 })
        .unwrap();
    objects
        .insert(tuple! { object_id: 4i64, size: 40i64 })
        .unwrap();
    objects
        .insert(tuple! { object_id: 5i64, size: 50i64 })
        .unwrap();

    let root_type =
        RelationType::new(TupleType::new().with_attribute("object_id", ScalarType::Int));
    let mut roots = Relation::new(root_type);
    roots.insert(tuple! { object_id: 1i64 }).unwrap();

    let ref_type = RelationType::new(
        TupleType::new()
            .with_attribute("from_id", ScalarType::Int)
            .with_attribute("to_id", ScalarType::Int),
    );
    let mut references = Relation::new(ref_type);
    // 1 -> 2
    references
        .insert(tuple! { from_id: 1i64, to_id: 2i64 })
        .unwrap();
    // 2 -> 3
    references
        .insert(tuple! { from_id: 2i64, to_id: 3i64 })
        .unwrap();
    // 4 -> 5 (Unreachable cycle or island)
    references
        .insert(tuple! { from_id: 4i64, to_id: 5i64 })
        .unwrap();
    references
        .insert(tuple! { from_id: 5i64, to_id: 4i64 })
        .unwrap();

    let garbage = sweep_garbage(&roots, &objects, &references).unwrap();

    assert_eq!(garbage.cardinality(), 2);
    // 4 and 5 should be garbage
    let mut garbage_ids: Vec<i64> = Vec::new();
    for t in garbage.into_iter() {
        garbage_ids.push(t.get_typed::<i64>("object_id").unwrap());
    }
    garbage_ids.sort();
    assert_eq!(garbage_ids, vec![4, 5]);
}
