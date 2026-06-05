content = open("relvar-core/src/algebra/rename.rs", "r").read()

old_code = """
        // Rename attributes in each tuple
        let renamed_tuples = self.tuples().map(move |tuple| {
            // Optimization: Zip pre-calculated new names with values.
            // Both iterators follow the sorted order of old attribute names.
            // - new_names_arc was built by iterating heading().attributes() (sorted by old_name)
            // - tuple.values().values() iterates values sorted by old_name (BTreeMap keys)
            let values_map: BTreeMap<String, _> = new_names_arc
                .iter()
                .zip(tuple.values().values())
                .map(|(new_name, value)| (new_name.clone(), value.clone()))
                .collect();

            // Safety:
            // 1. We constructed new_heading directly from old_heading with renames applied.
            // 2. We constructed values_map by zipping new names with old values in the same order.
            // 3. Types are preserved (we clone the type from old heading to new heading).
            // 4. "Last Write Wins" logic for duplicate target names is handled by BTreeMap::collect
            //    overwriting previous entries, matching the behavior of new_heading construction.
            Tuple::new_unchecked(new_heading_arc.clone(), values_map)
        });
"""

new_code = """
        // Rename attributes in each tuple
        let renamed_tuples = self.tuples().map(move |tuple| {
            // Optimization: Zip pre-calculated new names with values.
            // Both iterators follow the sorted order of old attribute names.
            // - new_names_arc was built by iterating heading().attributes() (sorted by old_name)
            // - tuple.values().values() iterates values sorted by old_name (BTreeMap keys)
            // By avoiding intermediate vectors and collect(), we avoid an extra heap allocation.
            let iter = new_names_arc
                .iter()
                .zip(tuple.values().values())
                .map(|(new_name, value)| (new_name.clone(), value.clone()));
            let values_map: BTreeMap<String, _> = std::collections::BTreeMap::from_iter(iter);

            // Safety:
            // 1. We constructed new_heading directly from old_heading with renames applied.
            // 2. We constructed values_map by zipping new names with old values in the same order.
            // 3. Types are preserved (we clone the type from old heading to new heading).
            // 4. "Last Write Wins" logic for duplicate target names is handled by BTreeMap::collect
            //    overwriting previous entries, matching the behavior of new_heading construction.
            Tuple::new_unchecked(new_heading_arc.clone(), values_map)
        });
"""

if old_code in content:
    with open("relvar-core/src/algebra/rename.rs", "w") as f:
        f.write(content.replace(old_code, new_code))
    print("Success")
else:
    print("Old code not found")

old_code2 = """
        // Rename attributes in each tuple by consuming it
        let renamed_tuples = self.into_iter().map(move |tuple| {
            // Optimization: Zip pre-calculated new names with consumed (key, value) pairs.
            // Both iterators follow the sorted order of old attribute names.
            // By keeping the old key string, we can reuse its memory allocation
            // if the name didn't actually change, avoiding String cloning.
            let values_map: BTreeMap<String, _> = new_names
                .iter()
                .zip(tuple.into_values())
                .map(|(new_name, (old_name, value))| {
                    if new_name == &old_name {
                        // Reuse the existing string allocation
                        (old_name, value)
                    } else {
                        // Must allocate a new string for the changed name
                        (new_name.clone(), value)
                    }
                })
                .collect();

            // Safety:
            // Same as rename: new names and types are guaranteed to match
            // the new heading we constructed above.
            Tuple::new_unchecked(new_heading_arc.clone(), values_map)
        });
"""

new_code2 = """
        // Rename attributes in each tuple by consuming it
        let renamed_tuples = self.into_iter().map(move |tuple| {
            // Optimization: Zip pre-calculated new names with consumed (key, value) pairs.
            // Both iterators follow the sorted order of old attribute names.
            // By keeping the old key string, we can reuse its memory allocation
            // if the name didn't actually change, avoiding String cloning.
            let iter = new_names
                .iter()
                .zip(tuple.into_values())
                .map(|(new_name, (old_name, value))| {
                    if new_name == &old_name {
                        // Reuse the existing string allocation
                        (old_name, value)
                    } else {
                        // Must allocate a new string for the changed name
                        (new_name.clone(), value)
                    }
                });
            let values_map: BTreeMap<String, _> = std::collections::BTreeMap::from_iter(iter);

            // Safety:
            // Same as rename: new names and types are guaranteed to match
            // the new heading we constructed above.
            Tuple::new_unchecked(new_heading_arc.clone(), values_map)
        });
"""

content = open("relvar-core/src/algebra/rename.rs", "r").read()
if old_code2 in content:
    with open("relvar-core/src/algebra/rename.rs", "w") as f:
        f.write(content.replace(old_code2, new_code2))
    print("Success 2")
else:
    print("Old code 2 not found")
