with open("relvar-core/src/constraints/foreign_key.rs", "r") as f:
    code = f.read()

new_code = code.replace(
"""    pub fn would_violate_on_insert(
        &self,
        new_tuple: &Tuple,
        referenced_relation: &Relation,
    ) -> Result<bool, ForeignKeyError> {
        let foreign_key_values: Vec<_> = self
            .foreign_key_attributes
            .iter()
            .map(|attr| new_tuple.get(attr).unwrap())
            .collect();

        // Check if any tuple in referenced relation matches
        for referenced_tuple in referenced_relation.tuples() {
            let referenced_values: Vec<_> = self
                .referenced_attributes
                .iter()
                .map(|attr| referenced_tuple.get(attr).unwrap())
                .collect();

            if foreign_key_values == referenced_values {
                return Ok(false); // Found a match, so no violation
            }
        }

        Ok(true) // No match found, violation
    }""",
"""    pub fn would_violate_on_insert(
        &self,
        new_tuple: &Tuple,
        referenced_relation: &Relation,
    ) -> Result<bool, ForeignKeyError> {
        let mut foreign_key_values = Vec::with_capacity(self.foreign_key_attributes.len());
        for attr in &self.foreign_key_attributes {
            foreign_key_values.push(new_tuple.get(attr).unwrap());
        }

        let mut referenced_values = Vec::with_capacity(self.referenced_attributes.len());

        // Check if any tuple in referenced relation matches
        for referenced_tuple in referenced_relation.tuples() {
            referenced_values.clear();
            for attr in &self.referenced_attributes {
                referenced_values.push(referenced_tuple.get(attr).unwrap());
            }

            if foreign_key_values == referenced_values {
                return Ok(false); // Found a match, so no violation
            }
        }

        Ok(true) // No match found, violation
    }""")

new_code = new_code.replace(
"""    pub fn would_violate_on_delete(
        &self,
        tuple_to_delete: &Tuple,
        referencing_relation: &Relation,
    ) -> Result<bool, ForeignKeyError> {
        let referenced_values: Vec<_> = self
            .referenced_attributes
            .iter()
            .map(|attr| tuple_to_delete.get(attr).unwrap())
            .collect();

        // Check if any tuple in referencing relation references this tuple
        for referencing_tuple in referencing_relation.tuples() {
            let foreign_key_values: Vec<_> = self
                .foreign_key_attributes
                .iter()
                .map(|attr| referencing_tuple.get(attr).unwrap())
                .collect();

            if foreign_key_values == referenced_values {
                return Ok(true);
            }
        }

        Ok(false)
    }""",
"""    pub fn would_violate_on_delete(
        &self,
        tuple_to_delete: &Tuple,
        referencing_relation: &Relation,
    ) -> Result<bool, ForeignKeyError> {
        let mut referenced_values = Vec::with_capacity(self.referenced_attributes.len());
        for attr in &self.referenced_attributes {
            referenced_values.push(tuple_to_delete.get(attr).unwrap());
        }

        let mut foreign_key_values = Vec::with_capacity(self.foreign_key_attributes.len());

        // Check if any tuple in referencing relation references this tuple
        for referencing_tuple in referencing_relation.tuples() {
            foreign_key_values.clear();
            for attr in &self.foreign_key_attributes {
                foreign_key_values.push(referencing_tuple.get(attr).unwrap());
            }

            if foreign_key_values == referenced_values {
                return Ok(true);
            }
        }

        Ok(false)
    }""")

with open("relvar-core/src/constraints/foreign_key.rs", "w") as f:
    f.write(new_code)

print("patched relvar-core/src/constraints/foreign_key.rs")
