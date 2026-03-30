with open("relvar-core/src/constraints/key.rs", "r") as f:
    code = f.read()

new_code = code.replace(
"""    /// Extract key value from a tuple
    fn extract_key_value(
        &self,
        tuple: &Tuple,
    ) -> Result<Vec<crate::values::ScalarValue>, KeyConstraintError> {
        let mut values = Vec::with_capacity(self.attributes.len());
        for attr in &self.attributes {
            if let Some(val) = tuple.get(attr) {
                values.push(val.clone());
            } else {
                return Err(KeyConstraintError::TupleMissingAttribute(attr.clone()));
            }
        }
        Ok(values)
    }""",
"""    /// Extract reference to key value from a tuple, avoiding clone
    fn extract_key_value_ref<'a>(
        &self,
        tuple: &'a Tuple,
        buffer: &mut Vec<&'a crate::values::ScalarValue>,
    ) -> Result<(), KeyConstraintError> {
        buffer.clear();
        for attr in &self.attributes {
            if let Some(val) = tuple.get(attr) {
                buffer.push(val);
            } else {
                return Err(KeyConstraintError::TupleMissingAttribute(attr.clone()));
            }
        }
        Ok(())
    }""")

new_code = new_code.replace(
"""    /// Check if this key is satisfied by a relation (all tuples have unique key values)
    pub fn is_satisfied_by(&self, relation: &Relation) -> Result<bool, KeyConstraintError> {
        // Verify all key attributes exist
        for attr in &self.attributes {
            if !relation.relation_type().has_attribute(attr) {
                return Err(KeyConstraintError::InvalidKeyAttributes(
                    self.attributes.clone(),
                ));
            }
        }

        // Collect all key values
        let mut key_values = HashSet::new();

        for tuple in relation.tuples() {
            let key_value = self.extract_key_value(tuple)?;
            if !key_values.insert(key_value) {
                // Duplicate found
                return Ok(false);
            }
        }

        Ok(true)
    }""",
"""    /// Check if this key is satisfied by a relation (all tuples have unique key values)
    pub fn is_satisfied_by(&self, relation: &Relation) -> Result<bool, KeyConstraintError> {
        // Verify all key attributes exist
        for attr in &self.attributes {
            if !relation.relation_type().has_attribute(attr) {
                return Err(KeyConstraintError::InvalidKeyAttributes(
                    self.attributes.clone(),
                ));
            }
        }

        // Collect all key values
        let mut key_values = HashSet::new();
        let mut buffer = Vec::with_capacity(self.attributes.len());

        for tuple in relation.tuples() {
            self.extract_key_value_ref(tuple, &mut buffer)?;
            if !key_values.insert(buffer.clone()) {
                // Duplicate found
                return Ok(false);
            }
        }

        Ok(true)
    }""")

new_code = new_code.replace(
"""    /// Check if a tuple would violate this key constraint when inserted into a relation
    pub fn would_violate(
        &self,
        relation: &Relation,
        new_tuple: &Tuple,
    ) -> Result<bool, KeyConstraintError> {
        let new_key_value = self.extract_key_value(new_tuple)?;

        for existing_tuple in relation.tuples() {
            let existing_key_value = self.extract_key_value(existing_tuple)?;
            if new_key_value == existing_key_value {
                return Ok(true);
            }
        }

        Ok(false)
    }""",
"""    /// Check if a tuple would violate this key constraint when inserted into a relation
    pub fn would_violate(
        &self,
        relation: &Relation,
        new_tuple: &Tuple,
    ) -> Result<bool, KeyConstraintError> {
        let mut new_key_value = Vec::with_capacity(self.attributes.len());
        self.extract_key_value_ref(new_tuple, &mut new_key_value)?;

        let mut existing_key_value = Vec::with_capacity(self.attributes.len());
        for existing_tuple in relation.tuples() {
            self.extract_key_value_ref(existing_tuple, &mut existing_key_value)?;
            if new_key_value == existing_key_value {
                return Ok(true);
            }
        }

        Ok(false)
    }""")

with open("relvar-core/src/constraints/key.rs", "w") as f:
    f.write(new_code)

print("patched relvar-core/src/constraints/key.rs")
