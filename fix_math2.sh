sed -i 's/sum += value;/sum = sum.checked_add(value).expect("sum overflow");/' relvar-core/src/algebra/summarize.rs
sed -i 's/total_capacity += rel.cardinality(),/total_capacity = total_capacity.checked_add(rel.cardinality()).expect("capacity overflow"),/' relvar-core/src/algebra/group.rs
