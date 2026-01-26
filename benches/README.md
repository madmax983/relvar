# Relvar Benchmarks

Comprehensive performance benchmarks for the Relvar RDBMS using [Criterion.rs](https://github.com/bheisler/criterion.rs).

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark suite
cargo bench --bench storage
cargo bench --bench algebra
cargo bench --bench database

# Run specific benchmark within a suite
cargo bench --bench storage -- heap_insert
cargo bench --bench algebra -- restrict
cargo bench --bench database -- query

# Generate reports without running benchmarks
cargo bench --no-run
```

## Benchmark Suites

### Storage Benchmarks (`benches/storage.rs`)

Tests the performance of the storage layer components.

**Page Operations:**
- `page_write` - Writing pages of various sizes (100, 500, 1000, 2000 bytes)
- `page_read` - Reading pages of various sizes

**Heap File Operations:**
- `heap_insert` - Inserting tuples (10, 50, 100, 500 tuples)
- `heap_scan` - Full table scans
- `heap_read_random` - Random tuple access by TupleId
- `heap_load_relation` - Loading entire heap file into a Relation

**Key Metrics:**
- Throughput: bytes/second for page operations
- Throughput: tuples/second for heap operations
- Latency: time per operation

### Algebra Benchmarks (`benches/algebra.rs`)

Tests the performance of relational algebra operators.

**Unary Operators:**
- `restrict` - Filtering tuples by predicate (100-5000 tuples)
- `project` - Selecting subset of attributes
- `rename` - Renaming attributes
- `extend` - Adding computed attributes

**Binary Operators:**
- `join` - Natural join between relations
- `union` - Set union
- `intersect` - Set intersection
- `difference` - Set difference

**Aggregation:**
- `summarize` - Grouping and aggregation (COUNT, AVG)

**Composite:**
- `chained_operations` - Realistic multi-operator queries

**Key Metrics:**
- Throughput: tuples/second processed
- Latency: time per operation
- Scalability: performance vs relation size

### Database Benchmarks (`benches/database.rs`)

Tests the performance of the complete database API.

**Database Operations:**
- `database_open` - Creating and opening databases
- `create_relvar` - Creating relation variables

**Data Manipulation:**
- `insert` - Inserting tuples (10-500 tuples)
- `insert_with_key_constraint` - Inserts with primary key checking
- `query` - Querying relations
- `query_with_operations` - Queries with restrict/project
- `delete` - Deleting tuples by predicate
- `update` - Updating tuples

**Transactions:**
- `transaction/commit` - Transaction commit overhead
- `transaction/rollback` - Transaction rollback overhead

**Realistic Workloads:**
- `realistic_workload/mixed_operations` - Mixed insert/query/update/delete

**Key Metrics:**
- Throughput: operations/second
- Latency: time per operation
- Constraint overhead: performance impact of constraints

## Benchmark Results

Results are saved in `target/criterion/` with:
- HTML reports with graphs
- Statistical analysis (mean, median, std dev)
- Comparison with previous runs
- Regression detection

### Viewing Reports

```bash
# Open the main report
open target/criterion/report/index.html

# View specific benchmark
open target/criterion/heap_insert/report/index.html
```

## CI Integration

Benchmarks run automatically in GitHub Actions:

1. **Weekly Schedule**: Every Sunday at 00:00 UTC
2. **Manual Trigger**: Via workflow_dispatch
3. **On Demand**: Include `[bench]` in commit message

Results are:
- Uploaded as artifacts (30 day retention)
- Posted as PR comments (when run on PRs)
- Stored in `target/criterion/` for local runs

## Performance Targets

Current target performance characteristics:

| Operation | Target | Notes |
|-----------|--------|-------|
| Page Write | > 50 MB/s | 4KB pages |
| Page Read | > 100 MB/s | Sequential reads |
| Heap Insert | > 10,000 tuples/s | Small tuples, no indexes |
| Restrict | > 1M tuples/s | Simple predicate |
| Project | > 500K tuples/s | 2-3 attributes |
| Join | > 10K tuples/s | Small relations |
| Query (simple) | < 1ms | Relations < 1000 tuples |
| Insert | > 1,000 ops/s | With constraints |

*Note: These are aspirational targets for an educational DBMS, not production requirements.*

## Interpreting Results

### Throughput

Higher is better. Measures operations per second or bytes per second.

### Latency (Time)

Lower is better. Time taken for a single operation.

### Statistical Confidence

Criterion provides:
- **Mean**: Average time across all iterations
- **Median**: Middle value (less affected by outliers)
- **Std Dev**: Variability in measurements
- **Confidence Intervals**: 95% confidence bounds

### Regression Detection

Criterion automatically detects performance regressions by comparing against baseline measurements. Look for:
- ⚠️ **Performance change detected** warnings
- Percentage changes > 5%
- Increased variability

## Optimization Guide

When optimizing based on benchmarks:

1. **Profile first**: Use `cargo flamegraph` or `perf` to identify hotspots
2. **Focus on bottlenecks**: Optimize the slowest operations first
3. **Measure impact**: Re-run benchmarks after changes
4. **Consider trade-offs**: Speed vs memory, simplicity vs performance
5. **Avoid premature optimization**: Optimize only when necessary

## Adding New Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_my_operation(c: &mut Criterion) {
    c.bench_function("my_operation", |b| {
        // Setup
        let data = setup_data();

        b.iter(|| {
            // Code to benchmark
            let result = my_operation(black_box(&data));
            black_box(result);
        });
    });
}

criterion_group!(benches, bench_my_operation);
criterion_main!(benches);
```

**Key practices:**
- Use `black_box()` to prevent compiler optimizations
- Separate setup from measured code
- Use `BenchmarkGroup` for parameterized benchmarks
- Set appropriate `Throughput` for meaningful metrics
- Keep benchmarks focused and isolated

## Troubleshooting

### Benchmarks Taking Too Long

```bash
# Reduce sample size
cargo bench -- --sample-size 10

# Quick run (less accurate)
cargo bench -- --quick
```

### Inconsistent Results

- Close other applications
- Run on AC power (not battery) for laptops to ensure consistent performance
- Use "High Performance" power plan if available
- Disable CPU frequency scaling
- Run multiple times and look at median

### Compilation Errors

```bash
# Clean and rebuild
cargo clean
cargo bench
```

## Resources

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Benchmarking Best Practices](https://easyperf.net/blog/2018/08/26/Microarchitectural-performance-events)
