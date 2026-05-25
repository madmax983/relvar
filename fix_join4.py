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

# semijoin.rs
semijoin_search_3 = """    pub fn semijoin_into(self, other: &Relation) -> Self {
        let common_attrs = common_attributes(other, other); // Safe to borrow from other to avoid moving out of self"""

semijoin_replace_3 = """    pub fn semijoin_into(self, other: &Relation) -> Self {
        let common_attrs = common_attributes(&self, other);"""

semijoin_search_4 = """    pub fn semidifference_into(self, other: &Relation) -> Self {
        let common_attrs = common_attributes(other, other); // Safe to borrow from other to avoid moving out of self"""

semijoin_replace_4 = """    pub fn semidifference_into(self, other: &Relation) -> Self {
        let common_attrs = common_attributes(&self, other);"""


replace_in_file('relvar-core/src/algebra/semijoin.rs', semijoin_search_3, semijoin_replace_3)
replace_in_file('relvar-core/src/algebra/semijoin.rs', semijoin_search_4, semijoin_replace_4)
