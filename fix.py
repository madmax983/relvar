import re

with open("relvar/src/experimental/spreadsheet.rs", "r") as f:
    content = f.read()

# Replace the struct-level docs to add an example
struct_replacement = """/// A Relational Spreadsheet Engine.
///
/// Models a spreadsheet where cells can contain raw values or formulas referencing
/// other cells. Evaluation is performed purely using relational joins and extensions
/// until all cell values are resolved (fixpoint).
///
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
/// use relvar::experimental::spreadsheet::Spreadsheet;
///
/// // 1. Define schema and relations
/// let val_heading = TupleType::new()
///     .with_attribute("id", ScalarType::String)
///     .with_attribute("val", ScalarType::Float);
/// let mut values = Relation::new(RelationType::new(val_heading));
///
/// let form_heading = TupleType::new()
///     .with_attribute("id", ScalarType::String)
///     .with_attribute("op", ScalarType::String)
///     .with_attribute("arg1", ScalarType::String)
///     .with_attribute("arg2", ScalarType::String);
/// let mut formulas = Relation::new(RelationType::new(form_heading));
///
/// // 2. Insert initial values
/// values.insert(tuple! { id: "A1".to_string(), val: 10.0f64 }).unwrap();
/// values.insert(tuple! { id: "A2".to_string(), val: 20.0f64 }).unwrap();
///
/// // 3. Insert formulas
/// // B1 = A1 + A2
/// formulas.insert(tuple! {
///     id: "B1".to_string(), op: "ADD".to_string(), arg1: "A1".to_string(), arg2: "A2".to_string()
/// }).unwrap();
///
/// // 4. Evaluate
/// let spreadsheet = Spreadsheet::new(values, formulas);
/// let result = spreadsheet.evaluate().unwrap();
///
/// // The result now contains A1, A2, and B1 (which is 30.0)
/// assert_eq!(result.cardinality(), 3);
/// ```
pub struct Spreadsheet {"""
content = content.replace("/// A Relational Spreadsheet Engine.\n///\n/// Models a spreadsheet where cells can contain raw values or formulas referencing\n/// other cells. Evaluation is performed purely using relational joins and extensions\n/// until all cell values are resolved (fixpoint).\npub struct Spreadsheet {", struct_replacement)

# Replace the new method docs
new_replacement = """    /// Creates a new Spreadsheet.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
    /// use relvar::experimental::spreadsheet::Spreadsheet;
    ///
    /// let val_heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::String)
    ///     .with_attribute("val", ScalarType::Float);
    /// let mut values = Relation::new(RelationType::new(val_heading));
    ///
    /// let form_heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::String)
    ///     .with_attribute("op", ScalarType::String)
    ///     .with_attribute("arg1", ScalarType::String)
    ///     .with_attribute("arg2", ScalarType::String);
    /// let mut formulas = Relation::new(RelationType::new(form_heading));
    ///
    /// let spreadsheet = Spreadsheet::new(values, formulas);
    /// ```
    pub fn new(values: Relation, formulas: Relation) -> Self {"""

content = re.sub(
    r"    /// Creates a new Spreadsheet\.\n    ///\n    /// # Examples\n    ///\n    /// ```\n    /// use relvar::\{Relation, RelationType, ScalarType, TupleType\};\n    /// use relvar::experimental::spreadsheet::Spreadsheet;\n    /// // Note: This is a placeholder example\n    /// ```\n    pub fn new\(values: Relation, formulas: Relation\) -> Self \{",
    new_replacement,
    content
)

# Replace the evaluate method docs
evaluate_replacement = """    /// Evaluates the spreadsheet until all possible formulas are resolved.
    ///
    /// This method performs relational joins and extensions to recursively evaluate
    /// formulas until a fixpoint is reached (no more formulas can be resolved).
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
    /// use relvar::experimental::spreadsheet::Spreadsheet;
    ///
    /// let val_heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::String)
    ///     .with_attribute("val", ScalarType::Float);
    /// let mut values = Relation::new(RelationType::new(val_heading));
    /// values.insert(tuple! { id: "A1".to_string(), val: 100.0f64 }).unwrap();
    /// values.insert(tuple! { id: "A2".to_string(), val: 50.0f64 }).unwrap();
    ///
    /// let form_heading = TupleType::new()
    ///     .with_attribute("id", ScalarType::String)
    ///     .with_attribute("op", ScalarType::String)
    ///     .with_attribute("arg1", ScalarType::String)
    ///     .with_attribute("arg2", ScalarType::String);
    /// let mut formulas = Relation::new(RelationType::new(form_heading));
    /// // A3 = A1 / A2
    /// formulas.insert(tuple! {
    ///     id: "A3".to_string(), op: "DIV".to_string(), arg1: "A1".to_string(), arg2: "A2".to_string()
    /// }).unwrap();
    ///
    /// let spreadsheet = Spreadsheet::new(values, formulas);
    /// let result = spreadsheet.evaluate().unwrap();
    ///
    /// // A3 is resolved to 2.0
    /// let a3 = result.tuples().find(|t| t.get_typed::<String>("id").unwrap() == "A3").unwrap();
    /// assert_eq!(a3.get_typed::<f64>("val").unwrap(), 2.0);
    /// ```
    pub fn evaluate(&self) -> Result<Relation, DatabaseError> {"""

content = re.sub(
    r"    /// Evaluates the spreadsheet until all possible formulas are resolved\.\n    ///\n    /// # Examples\n    ///\n    /// ```\n    /// use relvar::\{Relation, RelationType, ScalarType, TupleType\};\n    /// use relvar::experimental::spreadsheet::Spreadsheet;\n    /// // Note: This is a placeholder example\n    /// ```\n    pub fn evaluate\(&self\) -> Result<Relation, DatabaseError> \{",
    evaluate_replacement,
    content
)

with open("relvar/src/experimental/spreadsheet.rs", "w") as f:
    f.write(content)
