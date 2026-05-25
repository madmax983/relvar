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

# join.rs
join_search_3 = """fn perform_hash_join(
    build_rel: &Relation,
    probe_rel: &Relation,
    common_attrs: &[String],
    result_heading: &Arc<TupleType>,
) -> Result<HashSet<Tuple>, DatabaseError> {"""

join_replace_3 = """fn perform_hash_join(
    build_rel: &Relation,
    probe_rel: &Relation,
    common_attrs: &[&str],
    result_heading: &Arc<TupleType>,
) -> Result<HashSet<Tuple>, DatabaseError> {"""

replace_in_file('relvar-core/src/algebra/join.rs', join_search_3, join_replace_3)
