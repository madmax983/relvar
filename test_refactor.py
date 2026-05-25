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
join_search_1 = """fn compute_common_attributes(left: &Relation, right: &Relation) -> Vec<String> {
    left.relation_type()
        .heading()
        .attribute_names()
        .filter(|attr| right.relation_type().heading().has_attribute(attr))
        .cloned()
        .collect()
}"""

join_replace_1 = """fn compute_common_attributes<'a>(left: &'a Relation, right: &Relation) -> Vec<&'a str> {
    left.relation_type()
        .heading()
        .attribute_names()
        .filter(|attr| right.relation_type().heading().has_attribute(attr))
        .map(|s| s.as_str())
        .collect()
}"""

join_search_2 = """struct JoinKey<'t, 'a> {
    tuple: &'t Tuple,
    attributes: &'a [String],
}

impl<'t, 'a> PartialEq for JoinKey<'t, 'a> {
    fn eq(&self, other: &Self) -> bool {
        // We assume attributes are the same (or same values) as this is used internally
        // with the same common_attrs slice.
        for (i, attr) in self.attributes.iter().enumerate() {
            let v1 = self.tuple.get(attr);
            let v2 = other.tuple.get(&other.attributes[i]);
            if v1 != v2 {
                return false;
            }
        }
        true
    }
}

impl<'t, 'a> Hash for JoinKey<'t, 'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for attr in self.attributes {
            if let Some(val) = self.tuple.get(attr) {
                val.hash(state);
            }
        }
    }
}"""

join_replace_2 = """struct JoinKey<'t, 'a> {
    tuple: &'t Tuple,
    attributes: &'a [&'a str],
}

impl<'t, 'a> PartialEq for JoinKey<'t, 'a> {
    fn eq(&self, other: &Self) -> bool {
        // We assume attributes are the same (or same values) as this is used internally
        // with the same common_attrs slice.
        for (i, attr) in self.attributes.iter().enumerate() {
            let v1 = self.tuple.get(*attr);
            let v2 = other.tuple.get(other.attributes[i]);
            if v1 != v2 {
                return false;
            }
        }
        true
    }
}

impl<'t, 'a> Hash for JoinKey<'t, 'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for attr in self.attributes {
            if let Some(val) = self.tuple.get(*attr) {
                val.hash(state);
            }
        }
    }
}"""

# semijoin.rs
semijoin_search_1 = """fn common_attributes(a: &Relation, b: &Relation) -> Vec<String> {
    a.relation_type()
        .heading()
        .attribute_names()
        .filter(|attr| b.relation_type().heading().has_attribute(attr))
        .cloned()
        .collect()
}"""

semijoin_replace_1 = """fn common_attributes<'a>(a: &'a Relation, b: &Relation) -> Vec<&'a str> {
    a.relation_type()
        .heading()
        .attribute_names()
        .filter(|attr| b.relation_type().heading().has_attribute(attr))
        .map(|s| s.as_str())
        .collect()
}"""

semijoin_search_2 = """struct SemijoinKey<'t, 'a> {
    tuple: &'t Tuple,
    attributes: &'a [String],
}

impl<'t, 'a> PartialEq for SemijoinKey<'t, 'a> {
    fn eq(&self, other: &Self) -> bool {
        // We assume attributes are the same (or same values) as this is used internally
        // with the same common_attrs slice.
        for (i, attr) in self.attributes.iter().enumerate() {
            let v1 = self.tuple.get(attr);
            let v2 = other.tuple.get(&other.attributes[i]);
            if v1 != v2 {
                return false;
            }
        }
        true
    }
}

impl<'t, 'a> Hash for SemijoinKey<'t, 'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for attr in self.attributes {
            if let Some(val) = self.tuple.get(attr) {
                val.hash(state);
            }
        }
    }
}"""

semijoin_replace_2 = """struct SemijoinKey<'t, 'a> {
    tuple: &'t Tuple,
    attributes: &'a [&'a str],
}

impl<'t, 'a> PartialEq for SemijoinKey<'t, 'a> {
    fn eq(&self, other: &Self) -> bool {
        // We assume attributes are the same (or same values) as this is used internally
        // with the same common_attrs slice.
        for (i, attr) in self.attributes.iter().enumerate() {
            let v1 = self.tuple.get(*attr);
            let v2 = other.tuple.get(other.attributes[i]);
            if v1 != v2 {
                return false;
            }
        }
        true
    }
}

impl<'t, 'a> Hash for SemijoinKey<'t, 'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for attr in self.attributes {
            if let Some(val) = self.tuple.get(*attr) {
                val.hash(state);
            }
        }
    }
}"""

replace_in_file('relvar-core/src/algebra/join.rs', join_search_1, join_replace_1)
replace_in_file('relvar-core/src/algebra/join.rs', join_search_2, join_replace_2)

replace_in_file('relvar-core/src/algebra/semijoin.rs', semijoin_search_1, semijoin_replace_1)
replace_in_file('relvar-core/src/algebra/semijoin.rs', semijoin_search_2, semijoin_replace_2)
