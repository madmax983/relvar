#![cfg(feature = "nova")]
use relvar_core::experimental::build_system::BuildSystem;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Relation;

#[test]
fn test_build_system() {
    let dep_type = TupleType::new()
        .with_attribute("target", ScalarType::String)
        .with_attribute("dependency", ScalarType::String);
    let mut dependencies = Relation::new(RelationType::new(dep_type));
    dependencies
        .insert(tuple! { target: "main.o", dependency: "main.c" })
        .unwrap();
    dependencies
        .insert(tuple! { target: "main.o", dependency: "main.h" })
        .unwrap();
    dependencies
        .insert(tuple! { target: "lib.o", dependency: "lib.c" })
        .unwrap();
    dependencies
        .insert(tuple! { target: "lib.o", dependency: "lib.h" })
        .unwrap();
    dependencies
        .insert(tuple! { target: "app", dependency: "main.o" })
        .unwrap();
    dependencies
        .insert(tuple! { target: "app", dependency: "lib.o" })
        .unwrap();

    let time_type = TupleType::new()
        .with_attribute("file", ScalarType::String)
        .with_attribute("mtime", ScalarType::Int);
    let mut file_times = Relation::new(RelationType::new(time_type));
    // app is older than lib.o
    file_times
        .insert(tuple! { file: "app", mtime: 100i64 })
        .unwrap();
    file_times
        .insert(tuple! { file: "main.o", mtime: 90i64 })
        .unwrap();
    file_times
        .insert(tuple! { file: "lib.o", mtime: 110i64 })
        .unwrap();
    file_times
        .insert(tuple! { file: "main.c", mtime: 80i64 })
        .unwrap();
    file_times
        .insert(tuple! { file: "main.h", mtime: 80i64 })
        .unwrap();
    file_times
        .insert(tuple! { file: "lib.c", mtime: 80i64 })
        .unwrap();
    file_times
        .insert(tuple! { file: "lib.h", mtime: 80i64 })
        .unwrap();

    // app is stale because lib.o is newer (110 > 100).
    // lib.o is not stale because its dependencies are older.
    // main.o is not stale.
    // missing targets: none. Wait, missing targets from file_times?
    // Let's add a target missing from file_times
    dependencies
        .insert(tuple! { target: "missing.o", dependency: "missing.c" })
        .unwrap();
    file_times
        .insert(tuple! { file: "missing.c", mtime: 80i64 })
        .unwrap();

    let stale = BuildSystem::find_stale_targets(&dependencies, &file_times);

    assert_eq!(stale.cardinality(), 2); // app and missing.o

    let mut stale_names = Vec::new();
    for t in stale.tuples() {
        stale_names.push(t.get_typed::<String>("target").unwrap());
    }
    stale_names.sort();
    assert_eq!(stale_names, vec!["app", "missing.o"]);
}
