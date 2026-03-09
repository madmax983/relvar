use criterion::{black_box, criterion_group, criterion_main, Criterion};
use relvar_core::types::{ScalarType, TupleType, RelationType};
use relvar::tools::importer::from_csv;

fn benchmark_csv_import(c: &mut Criterion) {
    let mut csv_data = String::from("id,name,active,score\n");
    for i in 0..1000 {
        csv_data.push_str(&format!("{},\"User {}\",true,99.5\n", i, i));
    }

    let heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("active", ScalarType::Bool)
        .with_attribute("score", ScalarType::Float);
    let rel_type = RelationType::new(heading);

    c.bench_function("csv_import_1000", |b| {
        b.iter(|| {
            let relation = from_csv(
                black_box(csv_data.as_bytes()),
                black_box(rel_type.clone()),
                black_box(',')
            ).unwrap();
            black_box(relation);
        })
    });
}

criterion_group!(benches, benchmark_csv_import);
criterion_main!(benches);
