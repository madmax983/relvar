use criterion::{Criterion, black_box, criterion_group, criterion_main};
use relvar_core::values::ScalarValue;
use relvar_core::{ConstraintExpression, tuple};

fn bench_expression_in(c: &mut Criterion) {
    let mut values = Vec::new();
    for i in 0..100 {
        values.push(ScalarValue::Int(i));
    }
    let expr = ConstraintExpression::In("id".to_string(), values.into_iter().collect());
    let t1 = tuple! { id: 50i64 };
    let t2 = tuple! { id: 200i64 };

    c.bench_function("evaluate_in_hit", |b| {
        b.iter(|| {
            black_box(expr.evaluate(&t1).unwrap());
        })
    });

    c.bench_function("evaluate_in_miss", |b| {
        b.iter(|| {
            black_box(expr.evaluate(&t2).unwrap());
        })
    });
}

criterion_group!(benches, bench_expression_in);
criterion_main!(benches);
