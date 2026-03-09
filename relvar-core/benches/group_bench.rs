use criterion::{Criterion, black_box, criterion_group, criterion_main};
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, Tuple};
use std::sync::Arc;

fn bench_ungroup(c: &mut Criterion) {
    // Create a large relation to group then ungroup
    let heading = TupleType::new()
        .with_attribute("dept_id".to_string(), ScalarType::Int)
        .with_attribute("emp_id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String);

    let rel_type = RelationType::new(heading);
    let mut relation = Relation::new(rel_type);

    // 100 departments, 100 employees each -> 10,000 tuples
    for dept_id in 0..100 {
        for emp_id in 0..100 {
            relation
                .insert(tuple! {
                    dept_id: dept_id as i64,
                    emp_id: (dept_id * 1000 + emp_id) as i64,
                    name: format!("Employee {}", emp_id)
                })
                .unwrap();
        }
    }

    let grouped = relation.group(&["emp_id", "name"], "employees").unwrap();

    c.bench_function("ungroup_10000_tuples", |b| {
        b.iter(|| black_box(grouped.ungroup("employees").unwrap()))
    });
}

criterion_group!(benches, bench_ungroup);
criterion_main!(benches);
