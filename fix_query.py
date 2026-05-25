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

# query/mod.rs
query_search_1 = """    pub fn project<S: Into<String>>(self, attributes: Vec<S>) -> Self {
        Query::Project {
            input: Box::new(self),
            attributes: attributes.into_iter().map(|s| s.into()).collect(),
        }
    }"""

query_replace_1 = """    pub fn project<I, S>(self, attributes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Query::Project {
            input: Box::new(self),
            attributes: attributes.into_iter().map(|s| s.into()).collect(),
        }
    }"""

query_search_2 = """    pub fn rename<S1: Into<String>, S2: Into<String>>(self, mappings: Vec<(S1, S2)>) -> Self {
        Query::Rename {
            input: Box::new(self),
            mappings: mappings
                .into_iter()
                .map(|(a, b)| (a.into(), b.into()))
                .collect(),
        }
    }"""

query_replace_2 = """    pub fn rename<I, S1, S2>(self, mappings: I) -> Self
    where
        I: IntoIterator<Item = (S1, S2)>,
        S1: Into<String>,
        S2: Into<String>,
    {
        Query::Rename {
            input: Box::new(self),
            mappings: mappings
                .into_iter()
                .map(|(a, b)| (a.into(), b.into()))
                .collect(),
        }
    }"""

query_search_3 = """    pub fn summarize<S: Into<String>>(
        self,
        group_by: Vec<S>,
        aggregations: Vec<Aggregation>,
    ) -> Self {"""

query_replace_3 = """    pub fn summarize<I, S, A>(self, group_by: I, aggregations: A) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
        A: IntoIterator<Item = Aggregation>,
    {"""

query_search_4 = """        Query::Summarize {
            input: Box::new(self),
            group_by: group_by.into_iter().map(|s| s.into()).collect(),
            aggregations,
        }"""

query_replace_4 = """        Query::Summarize {
            input: Box::new(self),
            group_by: group_by.into_iter().map(|s| s.into()).collect(),
            aggregations: aggregations.into_iter().collect(),
        }"""

replace_in_file('relvar-core/src/query/mod.rs', query_search_1, query_replace_1)
replace_in_file('relvar-core/src/query/mod.rs', query_search_2, query_replace_2)
replace_in_file('relvar-core/src/query/mod.rs', query_search_3, query_replace_3)
replace_in_file('relvar-core/src/query/mod.rs', query_search_4, query_replace_4)
