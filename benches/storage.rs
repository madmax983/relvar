use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use relvar::storage::{HeapFile, Page, PageFile};
use relvar::tuple;
use relvar::types::{RelationType, ScalarType, TupleType};
use relvar::values::Tuple;
use tempfile::NamedTempFile;

fn create_test_relation_type() -> RelationType {
    let heading = TupleType::new()
        .with_attribute("id".to_string(), ScalarType::Int)
        .with_attribute("name".to_string(), ScalarType::String)
        .with_attribute("value".to_string(), ScalarType::Float);
    RelationType::new(heading)
}

fn create_test_tuple(id: i64) -> Tuple {
    tuple! {
        id: id,
        name: format!("Name_{}", id),
        value: (id as f64) * 1.5
    }
}

// Page benchmarks
fn bench_page_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("page_write");

    for size in [100, 500, 1000, 2000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let temp_file = NamedTempFile::new().unwrap();
                let mut page_file = PageFile::create(temp_file.path()).unwrap();
                let data = vec![0u8; size];
                let page = Page::from_data(0, data).unwrap();
                page_file.write_page(&page).unwrap();
                black_box(page_file);
            });
        });
    }
    group.finish();
}

fn bench_page_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("page_read");

    for size in [100, 500, 1000, 2000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let temp_file = NamedTempFile::new().unwrap();
            let mut page_file = PageFile::create(temp_file.path()).unwrap();
            let data = vec![42u8; size];
            let page = Page::from_data(0, data).unwrap();
            page_file.write_page(&page).unwrap();

            b.iter(|| {
                let result = page_file.read_page(0).unwrap();
                black_box(result);
            });
        });
    }
    group.finish();
}

// Heap file benchmarks
fn bench_heap_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("heap_insert");

    for count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            b.iter(|| {
                let temp_file = NamedTempFile::new().unwrap();
                let rel_type = create_test_relation_type();
                let mut heap = HeapFile::create(temp_file.path(), rel_type).unwrap();

                for i in 0..count {
                    let tuple = create_test_tuple(i as i64);
                    heap.insert_tuple(&tuple).unwrap();
                }
                black_box(heap);
            });
        });
    }
    group.finish();
}

fn bench_heap_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("heap_scan");

    for count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            let temp_file = NamedTempFile::new().unwrap();
            let rel_type = create_test_relation_type();
            let mut heap = HeapFile::create(temp_file.path(), rel_type).unwrap();

            // Pre-populate
            for i in 0..count {
                let tuple = create_test_tuple(i as i64);
                heap.insert_tuple(&tuple).unwrap();
            }

            b.iter(|| {
                let results = heap.scan().unwrap();
                black_box(results);
            });
        });
    }
    group.finish();
}

fn bench_heap_read_random(c: &mut Criterion) {
    let mut group = c.benchmark_group("heap_read_random");

    for count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            let temp_file = NamedTempFile::new().unwrap();
            let rel_type = create_test_relation_type();
            let mut heap = HeapFile::create(temp_file.path(), rel_type).unwrap();

            // Pre-populate and collect tuple IDs
            let mut tuple_ids = Vec::new();
            for i in 0..count {
                let tuple = create_test_tuple(i as i64);
                let tid = heap.insert_tuple(&tuple).unwrap();
                tuple_ids.push(tid);
            }

            b.iter(|| {
                // Read random tuples
                for tid in &tuple_ids {
                    let tuple = heap.read_tuple(*tid).unwrap();
                    black_box(tuple);
                }
            });
        });
    }
    group.finish();
}

fn bench_heap_load_relation(c: &mut Criterion) {
    let mut group = c.benchmark_group("heap_load_relation");

    for count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), count, |b, &count| {
            let temp_file = NamedTempFile::new().unwrap();
            let rel_type = create_test_relation_type();
            let mut heap = HeapFile::create(temp_file.path(), rel_type).unwrap();

            // Pre-populate
            for i in 0..count {
                let tuple = create_test_tuple(i as i64);
                heap.insert_tuple(&tuple).unwrap();
            }

            b.iter(|| {
                let relation = heap.load_relation().unwrap();
                black_box(relation);
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_page_write,
    bench_page_read,
    bench_heap_insert,
    bench_heap_scan,
    bench_heap_read_random,
    bench_heap_load_relation
);
criterion_main!(benches);
