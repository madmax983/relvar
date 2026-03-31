use criterion::{Criterion, black_box, criterion_group, criterion_main};
use relvar_core::{Relation, RelationType, ScalarType, ScalarValue, Tuple, TupleType};

fn create_test_relation(num_groups: usize, tuples_per_group: usize) -> Relation {
    let mut heading = TupleType::new();
    heading = heading.with_attribute("group_id".to_string(), ScalarType::Int);
    for i in 0..5 {
        heading = heading.with_attribute(format!("attr_{}", i), ScalarType::Int);
    }
    let rel_type = RelationType::new(heading.clone());
    let mut relation = Relation::new(rel_type);

    let heading_arc = std::sync::Arc::new(heading);

    for g in 0..num_groups {
        for t in 0..tuples_per_group {
            let mut values = std::collections::BTreeMap::new();
            values.insert("group_id".to_string(), ScalarValue::Int(g as i64));
            for i in 0..5 {
                values.insert(
                    format!("attr_{}", i),
                    ScalarValue::Int((g * tuples_per_group + t) as i64),
                );
            }
            relation
                .insert(Tuple::new(heading_arc.clone(), values).unwrap())
                .unwrap();
        }
    }
    relation
}

fn bench_group(c: &mut Criterion) {
    let relation = create_test_relation(100, 100); // 10,000 tuples

    let attrs_to_group = ["attr_0", "attr_1", "attr_2", "attr_3", "attr_4"];

    c.bench_function("group_10k_tuples", |b| {
        b.iter(|| black_box(relation.group(&attrs_to_group, "rva_name").unwrap()));
    });
}

criterion_group!(benches, bench_group);
criterion_main!(benches);
