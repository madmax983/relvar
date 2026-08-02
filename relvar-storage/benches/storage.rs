use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::Tuple;
use relvar_storage::{HeapFile, Page, PageFile};
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
            b.iter_batched(
                || {
                    // Setup: create file and page
                    let temp_file = NamedTempFile::new().unwrap();
                    let page_file = PageFile::create(temp_file.path()).unwrap();
                    let data = vec![0u8; size];
                    let page = Page::from_data(0, data).unwrap();
                    (page_file, page, temp_file)
                },
                |(mut page_file, page, _temp_file): (_, _, _)| {
                    // Measured: just the write operation
                    page_file.write_page(&page).unwrap();
                    black_box(page_file);
                },
                BatchSize::SmallInput,
            );
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
            b.iter_batched(
                || {
                    // Setup: create file and heap
                    let temp_file = NamedTempFile::new().unwrap();
                    let rel_type = create_test_relation_type();
                    let heap = HeapFile::create(temp_file.path(), rel_type).unwrap();
                    (heap, temp_file)
                },
                |(mut heap, _temp_file): (_, _)| {
                    // Measured: just the insert operations
                    for i in 0..count {
                        let tuple = create_test_tuple(i as i64);
                        heap.insert_tuple(&tuple).unwrap();
                    }
                    black_box(heap);
                },
                BatchSize::SmallInput,
            );
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

// bench_heap_read_random removed - used internal read_tuple(TupleId) API
// which is now pub(crate) per TTM Proscription 6. Random reads should
// be done via scan() with filtering in production code.

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
    // bench_heap_read_random, // REMOVED - used internal TupleId API
    bench_heap_load_relation
);
criterion_main!(benches);
