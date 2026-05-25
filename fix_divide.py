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

# divide.rs
divide_search_1 = """            let extended_values: BTreeMap<_, _> = candidate
                .values()
                .iter()
                .chain(divisor_tuple.values())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();"""

divide_replace_1 = """            let extended_values: BTreeMap<_, _> = candidate
                .values()
                .iter()
                .chain(divisor_tuple.values())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();"""

replace_in_file('relvar-core/src/algebra/divide.rs', divide_search_1, divide_replace_1)
