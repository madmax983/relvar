import re

def replace_in_file(filepath, search, replace):
    with open(filepath, 'r') as f:
        content = f.read()

    if search in content:
        content = content.replace(search, replace)
        with open(filepath, 'w') as f:
            f.write(content)
        print(f"Replaced in {filepath}")
    else:
        print(f"Not found in {filepath}")

# summarize.rs test optimizations
summarize_search_1 = """        // Check dept_id 10
        let dept10: Vec<_> = result
            .tuples()
            .filter(|t| t.get_typed::<i64>("dept_id").unwrap() == 10)
            .collect();
        assert_eq!(dept10.len(), 1);
        let dept10_tuple = dept10[0];"""

summarize_replace_1 = """        // Check dept_id 10
        let mut dept10_iter = result
            .tuples()
            .filter(|t| t.get_typed::<i64>("dept_id").unwrap() == 10);
        let dept10_tuple = dept10_iter.next().unwrap();
        assert!(dept10_iter.next().is_none());"""

summarize_search_2 = """        // Check dept_id 20
        let dept20: Vec<_> = result
            .tuples()
            .filter(|t| t.get_typed::<i64>("dept_id").unwrap() == 20)
            .collect();
        assert_eq!(dept20.len(), 1);
        let dept20_tuple = dept20[0];"""

summarize_replace_2 = """        // Check dept_id 20
        let mut dept20_iter = result
            .tuples()
            .filter(|t| t.get_typed::<i64>("dept_id").unwrap() == 20);
        let dept20_tuple = dept20_iter.next().unwrap();
        assert!(dept20_iter.next().is_none());"""


replace_in_file('relvar-core/src/algebra/summarize.rs', summarize_search_1, summarize_replace_1)
replace_in_file('relvar-core/src/algebra/summarize.rs', summarize_search_2, summarize_replace_2)
