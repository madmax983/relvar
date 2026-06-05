content = open("relvar-core/src/algebra/project.rs", "r").read()

new_code = """
    let mut tuple_iter = source_tuple.values().iter();
    let mut heading_iter = target_heading.attributes().iter();

    let mut current_tuple = tuple_iter.next();
    let mut current_heading = heading_iter.next();

    let iter = std::iter::from_fn(move || {
        loop {
            let (t_attr, t_val) = current_tuple?;
            let (h_attr, _) = current_heading?;

            use std::cmp::Ordering;
            match t_attr.cmp(h_attr) {
                Ordering::Equal => {
                    let res = (t_attr.clone(), t_val.clone());
                    current_tuple = tuple_iter.next();
                    current_heading = heading_iter.next();
                    return Some(res);
                }
                Ordering::Less => {
                    current_tuple = tuple_iter.next();
                }
                Ordering::Greater => {
                    current_heading = heading_iter.next();
                }
            }
        }
    });

    let values = std::collections::BTreeMap::from_iter(iter);
"""

old_code = """
    let mut tuple_iter = source_tuple.values().iter();
    let mut heading_iter = target_heading.attributes().iter();

    let mut current_tuple = tuple_iter.next();
    let mut current_heading = heading_iter.next();

    let mut result_items = Vec::with_capacity(target_heading.degree());

    while let (Some((t_attr, t_val)), Some((h_attr, _))) = (current_tuple, current_heading) {
        use std::cmp::Ordering;
        match t_attr.cmp(h_attr) {
            Ordering::Equal => {
                result_items.push((t_attr.clone(), t_val.clone()));
                current_tuple = tuple_iter.next();
                current_heading = heading_iter.next();
            }
            Ordering::Less => {
                current_tuple = tuple_iter.next();
            }
            Ordering::Greater => {
                current_heading = heading_iter.next();
            }
        }
    }

    let values = std::collections::BTreeMap::from_iter(result_items);
"""

if new_code in content:
    with open("relvar-core/src/algebra/project.rs", "w") as f:
        f.write(content.replace(new_code, old_code))
    print("Success")
else:
    print("New code not found")
