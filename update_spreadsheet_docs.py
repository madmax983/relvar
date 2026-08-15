with open('relvar/src/experimental/spreadsheet.rs', 'r') as f:
    content = f.read()

search_struct = """/// A Relational Spreadsheet Engine.
///
/// Models a spreadsheet where cells can contain raw values or formulas referencing
/// other cells. Evaluation is performed purely using relational joins and extensions
/// until all cell values are resolved (fixpoint).
pub struct Spreadsheet {"""

replace_struct = """/// A Relational Spreadsheet Engine.
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
/// // Create values relation: (id, val)
/// let mut values = Relation::new(RelationType::new(
///     TupleType::new()
///         .with_attribute("id", ScalarType::String)
///         .with_attribute("val", ScalarType::Float)
/// ));
/// values.insert(tuple! { id: "A1".to_string(), val: 10.0f64 }).unwrap();
///
/// // Create formulas relation: (id, op, arg1, arg2)
/// let mut formulas = Relation::new(RelationType::new(
///     TupleType::new()
///         .with_attribute("id", ScalarType::String)
///         .with_attribute("op", ScalarType::String)
///         .with_attribute("arg1", ScalarType::String)
///         .with_attribute("arg2", ScalarType::String)
/// ));
/// formulas.insert(tuple! {
///     id: "B1".to_string(), op: "ADD".to_string(), arg1: "A1".to_string(), arg2: "A1".to_string()
/// }).unwrap();
///
/// let spreadsheet = Spreadsheet::new(values, formulas);
/// ```
pub struct Spreadsheet {"""
content = content.replace(search_struct, replace_struct)

search_new = """    /// Creates a new Spreadsheet.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::spreadsheet::Spreadsheet;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(values: Relation, formulas: Relation) -> Self {"""

replace_new = """    /// Creates a new Spreadsheet.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
    /// use relvar::experimental::spreadsheet::Spreadsheet;
    ///
    /// let values = Relation::new(RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::String).with_attribute("val", ScalarType::Float)
    /// ));
    /// let formulas = Relation::new(RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::String)
    ///         .with_attribute("op", ScalarType::String)
    ///         .with_attribute("arg1", ScalarType::String)
    ///         .with_attribute("arg2", ScalarType::String)
    /// ));
    ///
    /// let spreadsheet = Spreadsheet::new(values, formulas);
    /// ```
    pub fn new(values: Relation, formulas: Relation) -> Self {"""
content = content.replace(search_new, replace_new)

search_eval = """    /// Evaluates the spreadsheet until all possible formulas are resolved.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::spreadsheet::Spreadsheet;
    /// // Note: This is a placeholder example
    /// ```
    pub fn evaluate(&self) -> Result<Relation, DatabaseError> {"""

replace_eval = """    /// Evaluates the spreadsheet until all possible formulas are resolved.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
    /// use relvar::experimental::spreadsheet::Spreadsheet;
    ///
    /// let mut values = Relation::new(RelationType::new(
    ///     TupleType::new().with_attribute("id", ScalarType::String).with_attribute("val", ScalarType::Float)
    /// ));
    /// values.insert(tuple! { id: "A1".to_string(), val: 10.0f64 }).unwrap();
    /// values.insert(tuple! { id: "A2".to_string(), val: 20.0f64 }).unwrap();
    ///
    /// let mut formulas = Relation::new(RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("id", ScalarType::String)
    ///         .with_attribute("op", ScalarType::String)
    ///         .with_attribute("arg1", ScalarType::String)
    ///         .with_attribute("arg2", ScalarType::String)
    /// ));
    /// // B1 = A1 + A2
    /// formulas.insert(tuple! {
    ///     id: "B1".to_string(), op: "ADD".to_string(), arg1: "A1".to_string(), arg2: "A2".to_string()
    /// }).unwrap();
    ///
    /// let spreadsheet = Spreadsheet::new(values, formulas);
    /// let result = spreadsheet.evaluate().unwrap();
    /// assert_eq!(result.cardinality(), 3); // A1, A2, and B1
    /// ```
    pub fn evaluate(&self) -> Result<Relation, DatabaseError> {"""
content = content.replace(search_eval, replace_eval)

with open('relvar/src/experimental/spreadsheet.rs', 'w') as f:
    f.write(content)
