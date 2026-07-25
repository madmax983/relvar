//! A relational approach to evaluating spreadsheet formulas.
//!
//! This module demonstrates how a seemingly imperative domain (spreadsheet formula evaluation)
//! can be elegantly modeled using pure relational algebra. By treating cell values and formulas
//! as relations, we can evaluate the spreadsheet by iteratively joining them until a fixpoint is reached.
//!
//! Instead of a directed acyclic graph (DAG) of cell dependencies, we represent the spreadsheet state
//! as two relations: `values` (resolved cells) and `formulas` (unresolved cells). The evaluation
//! process repeatedly joins `formulas` with `values` to compute new cell values, and unions them
//! back into `values` until no new formulas can be resolved.

use relvar_core::{
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Spreadsheet Engine.
///
/// Models a spreadsheet where cells can contain raw values or formulas referencing
/// other cells. Evaluation is performed purely using relational joins and extensions
/// until all cell values are resolved (fixpoint).
///
/// # Examples
///
/// ```
/// use relvar_core::{Relation, RelationType, TupleType, tuple, types::ScalarType};
/// use relvar::experimental::spreadsheet::Spreadsheet;
///
/// let val_heading = TupleType::new()
///     .with_attribute("id", ScalarType::String)
///     .with_attribute("val", ScalarType::Float);
/// let mut values = Relation::new(RelationType::new(val_heading));
/// values.insert(tuple! { id: "A1".to_string(), val: 10.0f64 }).unwrap();
///
/// let form_heading = TupleType::new()
///     .with_attribute("id", ScalarType::String)
///     .with_attribute("op", ScalarType::String)
///     .with_attribute("arg1", ScalarType::String)
///     .with_attribute("arg2", ScalarType::String);
/// let formulas = Relation::new(RelationType::new(form_heading));
///
/// let sheet = Spreadsheet::new(values, formulas);
/// ```
pub struct Spreadsheet {
    /// Resolved values. Schema: `(id: String, val: Float)`
    pub values: Relation,
    /// Formulas. Schema: `(id: String, op: String, arg1: String, arg2: String)`
    /// Supported ops: "ADD", "SUB", "MUL", "DIV"
    pub formulas: Relation,
}

impl Spreadsheet {
    /// Creates a new Spreadsheet.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{Relation, RelationType, TupleType, types::ScalarType};
    /// use relvar::experimental::spreadsheet::Spreadsheet;
    ///
    /// let val_heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::String)
    ///     .with_attribute("val", ScalarType::Float);
    /// let values = Relation::new(RelationType::new(val_heading));
    ///
    /// let form_heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::String)
    ///     .with_attribute("op", ScalarType::String)
    ///     .with_attribute("arg1", ScalarType::String)
    ///     .with_attribute("arg2", ScalarType::String);
    /// let formulas = Relation::new(RelationType::new(form_heading));
    ///
    /// let sheet = Spreadsheet::new(values, formulas);
    /// ```
    pub fn new(values: Relation, formulas: Relation) -> Self {
        Self { values, formulas }
    }

    /// Evaluates the spreadsheet until all possible formulas are resolved.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{Relation, RelationType, TupleType, tuple, types::ScalarType};
    /// use relvar::experimental::spreadsheet::Spreadsheet;
    ///
    /// let val_heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::String)
    ///     .with_attribute("val", ScalarType::Float);
    /// let mut values = Relation::new(RelationType::new(val_heading));
    /// values.insert(tuple! { id: "A1".to_string(), val: 10.0f64 }).unwrap();
    /// values.insert(tuple! { id: "A2".to_string(), val: 20.0f64 }).unwrap();
    ///
    /// let form_heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::String)
    ///     .with_attribute("op", ScalarType::String)
    ///     .with_attribute("arg1", ScalarType::String)
    ///     .with_attribute("arg2", ScalarType::String);
    /// let mut formulas = Relation::new(RelationType::new(form_heading));
    /// formulas.insert(tuple! {
    ///     id: "B1".to_string(), op: "ADD".to_string(), arg1: "A1".to_string(), arg2: "A2".to_string()
    /// }).unwrap();
    ///
    /// let sheet = Spreadsheet::new(values, formulas);
    /// let resolved = sheet.evaluate().unwrap();
    /// assert_eq!(resolved.cardinality(), 3); // A1, A2, and B1
    /// ```
    pub fn evaluate(&self) -> Result<Relation, DatabaseError> {
        let mut current_values = self.values.clone();

        loop {
            let initial_count = current_values.cardinality();

            // Find unresolved formulas by antijoining/difference with resolved values
            // We only want formulas whose 'id' is NOT yet in current_values
            let resolved_ids = current_values.project(&["id"]);
            let formula_ids = self.formulas.project(&["id"]);

            let unresolved_ids = formula_ids
                .difference(&resolved_ids)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            let unresolved_formulas = self.formulas.join(&unresolved_ids)?;

            // Rename current_values for arg1
            let values_arg1 = current_values.rename(&[("id", "arg1"), ("val", "val1")]);
            // Rename current_values for arg2
            let values_arg2 = current_values.rename(&[("id", "arg2"), ("val", "val2")]);

            // Join unresolved formulas with arg1 values
            let with_arg1 = unresolved_formulas.join(&values_arg1)?;
            // Join with arg2 values
            let fully_resolved_args = with_arg1.join(&values_arg2)?;

            // Evaluate the formula
            let evaluated = fully_resolved_args
                .extend("val", ScalarType::Float, |t| {
                    let op = t.get_typed::<String>("op").unwrap();
                    let val1 = t.get_typed::<f64>("val1").unwrap();
                    let val2 = t.get_typed::<f64>("val2").unwrap();

                    let result = match op.as_str() {
                        "ADD" => val1 + val2,
                        "SUB" => val1 - val2,
                        "MUL" => val1 * val2,
                        "DIV" => {
                            if val2 != 0.0 {
                                val1 / val2
                            } else {
                                f64::NAN
                            }
                        }
                        _ => f64::NAN,
                    };
                    ScalarValue::Float(result)
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Project back to (id, val)
            let new_values = evaluated.project(&["id", "val"]);

            // Union with current values
            current_values = current_values
                .union(&new_values)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            if current_values.cardinality() == initial_count {
                break;
            }
        }

        Ok(current_values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_spreadsheet_evaluation() {
        let val_heading = TupleType::new()
            .with_attribute("id", ScalarType::String)
            .with_attribute("val", ScalarType::Float);
        let mut values = Relation::new(RelationType::new(val_heading));

        // A1 = 10, A2 = 20
        values
            .insert(tuple! { id: "A1".to_string(), val: 10.0f64 })
            .unwrap();
        values
            .insert(tuple! { id: "A2".to_string(), val: 20.0f64 })
            .unwrap();

        let form_heading = TupleType::new()
            .with_attribute("id", ScalarType::String)
            .with_attribute("op", ScalarType::String)
            .with_attribute("arg1", ScalarType::String)
            .with_attribute("arg2", ScalarType::String);
        let mut formulas = Relation::new(RelationType::new(form_heading));

        // B1 = A1 + A2
        formulas.insert(tuple! {
            id: "B1".to_string(), op: "ADD".to_string(), arg1: "A1".to_string(), arg2: "A2".to_string()
        }).unwrap();

        // C1 = B1 * A2
        formulas.insert(tuple! {
            id: "C1".to_string(), op: "MUL".to_string(), arg1: "B1".to_string(), arg2: "A2".to_string()
        }).unwrap();

        let spreadsheet = Spreadsheet::new(values, formulas);
        let result = spreadsheet.evaluate().unwrap();

        assert_eq!(result.cardinality(), 4); // A1, A2, B1, C1

        let b1 = result
            .tuples()
            .find(|t| t.get_typed::<String>("id").unwrap() == "B1")
            .unwrap();
        assert_eq!(b1.get_typed::<f64>("val").unwrap(), 30.0);

        let c1 = result
            .tuples()
            .find(|t| t.get_typed::<String>("id").unwrap() == "C1")
            .unwrap();
        assert_eq!(c1.get_typed::<f64>("val").unwrap(), 600.0); // 30 * 20
    }
}
