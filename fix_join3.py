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
join_search_4 = """fn build_join_map<'t, 'a>(
    build_rel: &'t Relation,
    common_attrs: &'a [String],
) -> Result<HashMap<JoinKey<'t, 'a>, Vec<&'t Tuple>>, DatabaseError> {"""

join_replace_4 = """fn build_join_map<'t, 'a>(
    build_rel: &'t Relation,
    common_attrs: &'a [&'a str],
) -> Result<HashMap<JoinKey<'t, 'a>, Vec<&'t Tuple>>, DatabaseError> {"""


join_search_5 = """fn probe_and_combine<'t, 'a>(
    probe_rel: &'t Relation,
    build_map: &HashMap<JoinKey<'t, 'a>, Vec<&'t Tuple>>,
    common_attrs: &'a [String],
    result_heading: &Arc<TupleType>,
) -> Result<HashSet<Tuple>, DatabaseError> {"""

join_replace_5 = """fn probe_and_combine<'t, 'a>(
    probe_rel: &'t Relation,
    build_map: &HashMap<JoinKey<'t, 'a>, Vec<&'t Tuple>>,
    common_attrs: &'a [&'a str],
    result_heading: &Arc<TupleType>,
) -> Result<HashSet<Tuple>, DatabaseError> {"""


semijoin_search_5 = """        let t1 = tuple! { a: 1i64 };
        let attributes = vec!["a".to_string(), "b".to_string()];

        let key1 = super::SemijoinKey {
            tuple: &t1,
            attributes: &attributes,
        };"""

semijoin_replace_5 = """        let t1 = tuple! { a: 1i64 };
        let attributes = vec!["a", "b"];

        let key1 = super::SemijoinKey {
            tuple: &t1,
            attributes: &attributes,
        };"""


replace_in_file('relvar-core/src/algebra/join.rs', join_search_4, join_replace_4)
replace_in_file('relvar-core/src/algebra/join.rs', join_search_5, join_replace_5)
replace_in_file('relvar-core/src/algebra/semijoin.rs', semijoin_search_5, semijoin_replace_5)
