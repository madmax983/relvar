1. **Refactor `Tuple::new` in `relvar-core/src/values/tuple.rs`:**
    - It uses a manual `for` loop to check if the length of collected values matches the degree. If not, it iterates over all attributes to find missing values.
    - We can refactor this by using iterators. Instead of using `contains_key`, we can find the missing attribute using iterator operations.
    - Also, `if let Some` can be changed to early returns using `else` branch.
    - Actually, `Tuple::new` is already pretty good, let's look for other places.

Wait, `Tuple::new` can definitely be flattened and simplified:
```rust
        if values_map.len() != tuple_type.degree() {
            if let Some(missing_attr) = tuple_type
                .attribute_names()
                .find(|attr_name| !values_map.contains_key(*attr_name))
            {
                return Err(TupleError::MissingValue(missing_attr.clone()));
            }
        }
```
Currently it has:
```rust
        if values_map.len() != tuple_type.degree() {
            for attr_name in tuple_type.attribute_names() {
                if !values_map.contains_key(attr_name) {
                    return Err(TupleError::MissingValue(attr_name.clone()));
                }
            }
        }
```

Wait, what about `validate_type_constraints` in `manager.rs`?
```rust
    pub fn validate_type_constraints(
        &self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), ConstraintManagerError> {
        let Some(attr_constraints) = self.type_constraints.get(relation_name) else {
            return Ok(());
        };

        for (attr_name, constraints) in attr_constraints {
            let Some(value) = tuple.get(attr_name) else { continue; }; // or whatever it should do
            if !constraints.is_satisfied_by(value).map_err(|e| {
                ConstraintManagerError::TypeConstraintViolation(e.to_string())
            })? {
                return Err(ConstraintManagerError::TypeConstraintViolation(format!(
                    "Attribute {} violates constraint",
                    attr_name
                )));
            }
        }
        Ok(())
    }
```
Currently:
```rust
        if let Some(attr_constraints) = self.type_constraints.get(relation_name) {
            for (attr_name, constraints) in attr_constraints {
                if let Some(value) = tuple.get(attr_name)
                    && !constraints.is_satisfied_by(value).map_err(|e| {
                        ConstraintManagerError::TypeConstraintViolation(e.to_string())
                    })?
                {
                    return Err(ConstraintManagerError::TypeConstraintViolation(format!(
                        "Attribute {} violates constraint",
                        attr_name
                    )));
                }
            }
        }
```
The early return (guard clause) style is strongly encouraged by Forge.

Let's check `if_let_some.txt` again for more `if let Some`. There are many places where guard clauses (`let Some(x) = y else { return; }`) can be used to flatten the nesting. Let's look through `manager.rs` and apply guard clauses where appropriate.

Another one in `manager.rs`:
```rust
    pub fn validate_check_constraints(
        &self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), ConstraintManagerError> {
        if let Some(check_constraints) = self.check_constraints.get(relation_name) {
            check_constraints.are_all_satisfied_by(tuple)?;
        }
        Ok(())
    }
```
Can be refactored to:
```rust
    pub fn validate_check_constraints(
        &self,
        relation_name: &str,
        tuple: &Tuple,
    ) -> Result<(), ConstraintManagerError> {
        let Some(check_constraints) = self.check_constraints.get(relation_name) else {
            return Ok(());
        };
        check_constraints.are_all_satisfied_by(tuple)?;
        Ok(())
    }
```

And in `validate_key_constraints_single_tuple`:
```rust
        if let Some(key_constraints) = self.key_constraints.get(relation_name) {
            if let Some(pk) = key_constraints.primary_key()
                && pk
                    .would_violate(current_relation, tuple)
                    .map_err(|e| ConstraintManagerError::TransactionError(e.to_string()))?
            {
                return Err(ConstraintManagerError::PrimaryKeyViolation);
            }

            for ck in key_constraints.candidate_keys() {
...
```
Can be:
```rust
        let Some(key_constraints) = self.key_constraints.get(relation_name) else {
            return Ok(());
        };
```
This flattens the whole function.

Let's check `database/data.rs`:
```rust
    pub fn query(&self, relation_name: &str) -> Result<Relation, DatabaseError> {
        // Check if this is a virtual relvar
        if let Some(virtual_relvar) = self.virtual_relvars.get(relation_name) {
            return (virtual_relvar.evaluator)(self);
        }

        // Otherwise, load from engine
        Ok(self.engine.load_relation(relation_name)?)
    }
```
This is fine.

```rust
    pub(crate) fn validate_relation_constraints(
        &mut self,
        relation_name: &str,
        relation: &Relation,
    ) -> Result<(), DatabaseError> {
        // Validate key constraints on new relation
        if let Some(key_constraints) = self.constraints.get_key_constraints(relation_name) {
            self.constraints
                .validate_key_constraints_bulk(relation, key_constraints)?;
        }
```
Could use `let Some(...) else`.

Let's do a refactor of `ConstraintManager` in `relvar-core/src/constraints/manager.rs` using Forge's guard clauses. This flattens nesting.

Also, `relvar-core/src/database/schema.rs`:
```rust
    pub fn get_relvar_type(&self, name: &str) -> Result<RelationType, DatabaseError> {
        // Check virtual relvars first
        if let Some(def) = self.virtual_relvars.get(name) {
            return Ok(def.relation_type.clone());
        }

        // Check base relvars
        let metadata = self.engine.get_relation_metadata(name)?;
        Ok(metadata.relation_type)
    }
```
Fine.

Let's check `Tuple::new` in `relvar-core/src/values/tuple.rs`:
```rust
        for (attr_name, value) in values {
            let Some(expected_type) = tuple_type.get_attribute_type(&attr_name) else {
                return Err(TupleError::AttributeNotFound(attr_name));
            };
            if !value.is_type(expected_type) {
                return Err(TupleError::TypeMismatch(
                    attr_name,
                    expected_type.name().to_string(),
                    value.scalar_type().name().to_string(),
                ));
            }
            values_map.insert(attr_name, value);
        }

        // Verify all attributes have values. Optimize by checking length first.
        if values_map.len() != tuple_type.degree() {
            for attr_name in tuple_type.attribute_names() {
                if !values_map.contains_key(attr_name) {
                    return Err(TupleError::MissingValue(attr_name.clone()));
                }
            }
        }
```
Let's see `conforms_to`:
```rust
        for (attr_name, value) in &self.values {
            let Some(expected_type) = tuple_type.get_attribute_type(attr_name) else {
                return false;
            };
            if !value.is_type(expected_type) {
                return false;
            }
        }
```

These changes follow Forge's preference for early returns (`let else`).

Wait, what else?
Are there long functions? `create_relation` in `relvar-core/src/storage_engine/mod.rs`? Wait, `create_relation` is a trait method, it's not a function with a body, but maybe there's a default implementation? Ah, `create_relation` in the script output said "66 lines" at `relvar-core/src/storage_engine/mod.rs:100` but it's just a trait definition. The python script counts from `fn` to `}` so it incorrectly counts the trait method and subsequent methods if they are just signatures without blocks? Oh, the python script had a bug `open_b = line.count('{') - line.count('}')` but a trait signature `fn foo();` has 0 `{`. The script handles `open_b > 0 or not started`, but since `started` is False and there are no `{`, it just keeps reading until it finds a `{` which might be in the next function with a default impl.

Let's look at `setup_db` in `relvar-core/src/query/tests/common.rs`, it's test code.
`compute_ungrouped_tuples` in `relvar-core/src/algebra/group.rs:415`:
```rust
fn compute_ungrouped_tuples(
    relation: &Relation,
    rva_name: &str,
    result_heading: &TupleType,
    _rva_relation_type: &RelationType,
) -> Result<std::collections::HashSet<Tuple>, UngroupError> {
    // Perform an initial pass to sum the cardinality of the target RVAs
    // to pre-allocate the exact needed capacity, avoiding dynamic heap reallocations.
    let mut total_capacity = 0;
    for tuple in relation.tuples() {
        let val = tuple
            .get(rva_name)
            .ok_or_else(|| UngroupError::AttributeNotFound(rva_name.to_string()))?;
        match val {
            ScalarValue::Relation(rel) => total_capacity += rel.cardinality(),
            _ => return Err(UngroupError::NotRelationValued(rva_name.to_string())),
        }
    }
    let mut result_tuples = std::collections::HashSet::with_capacity(total_capacity);
    let result_heading_arc = std::sync::Arc::new(result_heading.clone());

    for tuple in relation.tuples() {
        // Get the RVA relation
        let val = tuple
            .get(rva_name)
            .ok_or_else(|| UngroupError::AttributeNotFound(rva_name.to_string()))?;
        let rva_relation = match val {
            ScalarValue::Relation(rel) => rel,
            _ => return Err(UngroupError::NotRelationValued(rva_name.to_string())),
        };

        // Pre-compute the invariant non-RVA attributes for this outer tuple
        let mut base_values = std::collections::BTreeMap::new();
        for (attr_name, val) in tuple.values().iter() {
            if attr_name != rva_name {
                base_values.insert(attr_name.clone(), val.clone());
            }
        }

        // For each tuple in the RVA, create a new tuple combining non-RVA and RVA attributes
        let heading_arc = result_heading_arc.clone();
        for rva_tuple in rva_relation.tuples() {
            let mut values = base_values.clone();
            for (attr_name, val) in rva_tuple.values().iter() {
                values.insert(attr_name.clone(), val.clone());
            }

            // Re-use the cloned heading_arc
            let result_tuple = Tuple::new_unchecked(heading_arc.clone(), values);
            result_tuples.insert(result_tuple);
        }
    }

    Ok(result_tuples)
}
```
This uses manual loops, we can use iterator chains. But performance was already optimized by Bolt. Let's see if we can use guard clauses to refactor `manager.rs`.

Let's read `relvar-core/src/constraints/manager.rs` fully to see what we can refactor.
