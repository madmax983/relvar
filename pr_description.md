Title: ⚡ Bolt: Eliminate redundant Vec allocations for BTreeMap construction

💡 **What:** Avoided redundant `Vec` allocations by converting multiple relational algebra operators (`project` and `join`) to use custom iterators via `std::iter::from_fn` and `.map()` instead of pushing elements to an intermediate vector before building a `BTreeMap`.

🎯 **Why:** Several methods (`combine_tuples` in `join`, `project_tuple_values` in `project`) were constructing a `Vec` for building tuples or attributes, only to consume the vector immediately to produce a `BTreeMap` or iterate over it. This caused an unnecessary intermediate heap allocation and iteration step.

📊 **Impact:** Reduces heap allocations by bypassing the intermediate `Vec` allocations during `project` and `join` operations. This is a zero-cost abstraction improvement utilizing iterators and `BTreeMap::from_iter`.

🔬 **Measurement:** Verify with `cargo test` and `cargo bench --package relvar-core`. Tests confirm correct behavioral preservation.
