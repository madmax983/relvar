import re

with open("relvar-core/src/algebra/rename.rs", "r") as f:
    content = f.read()

rename_into_impl = """
    /// Renames attributes in this relation according to the provided mapping, consuming the relation.
    ///
    /// This is an optimized version of `rename` that avoids allocating a new
    /// collection for tuples by mutating them in-place when possible.
    pub fn rename_into(self, mappings: &[(&str, &str)]) -> Self {
        let mut new_heading = TupleType::new();
        let mut new_names = Vec::with_capacity(self.relation_type().heading().degree());

        for (old_name, attr_type) in self.relation_type().heading().attributes() {
            let new_name = mappings
                .iter()
                .find(|(from, _)| from == old_name)
                .map(|(_, to)| *to)
                .unwrap_or(old_name.as_str());

            new_heading = new_heading.with_attribute(new_name, attr_type.clone());
            new_names.push(new_name.to_string());
        }

        let new_rel_type = RelationType::new(new_heading.clone());
        let new_heading_arc = Arc::new(new_heading);
        let new_names_arc = Arc::new(new_names);

        let renamed_tuples = self.into_iter().map(move |tuple| {
            let mut values_map = BTreeMap::new();
            for (new_name, (_, value)) in new_names_arc.iter().zip(tuple.into_values().into_iter()) {
                values_map.insert(new_name.clone(), value);
            }
            Tuple::new_unchecked(new_heading_arc.clone(), values_map)
        });

        Relation::from_tuples_unchecked(new_rel_type, renamed_tuples)
    }
"""

content = content.replace("    }\n}\n\n#[cfg(test)]", "    }\n" + rename_into_impl + "}\n\n#[cfg(test)]")

with open("relvar-core/src/algebra/rename.rs", "w") as f:
    f.write(content)
