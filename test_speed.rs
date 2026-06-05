use std::time::Instant;
use std::iter::FromIterator;

fn main() {
    let mut source = vec![];
    for i in 0..1000 {
        source.push((i.to_string(), i));
    }

    // old
    let start = Instant::now();
    for _ in 0..1000 {
        let mut result_items = Vec::with_capacity(3);
        for i in 0..source.len() {
            if i % 300 == 0 {
                result_items.push((source[i].0.clone(), source[i].1.clone()));
            }
        }
        let values = std::collections::BTreeMap::from_iter(result_items);
    }
    println!("Old: {:?}", start.elapsed());

    // new
    let start = Instant::now();
    for _ in 0..1000 {
        let mut iter = source.iter().enumerate().filter(|(i, _)| *i % 300 == 0).map(|(_, x)| (x.0.clone(), x.1.clone()));
        let iter = std::iter::from_fn(move || {
            iter.next()
        });
        let values = std::collections::BTreeMap::from_iter(iter);
    }
    println!("New: {:?}", start.elapsed());
}
