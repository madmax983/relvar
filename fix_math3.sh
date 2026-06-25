sed -i 's/let mut total_capacity = 0;/let mut total_capacity: usize = 0;/' relvar-core/src/algebra/group.rs
sed -i 's/sum = sum.checked_add(value).expect("sum overflow");/sum += value;/' relvar-core/src/algebra/summarize.rs
