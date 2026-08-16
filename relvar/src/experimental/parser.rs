//! Relational CYK Parser
//!
//! This module demonstrates how the Cocke-Younger-Kasami (CYK) parsing algorithm
//! for Context-Free Grammars (CFG) in Chomsky Normal Form (CNF) can be implemented
//! using purely relational algebra.
//!
//! # Concept
//!
//! A grammar in CNF consists of rules of two types:
//! - Terminal rules: `A -> a`
//! - Non-terminal rules: `A -> B C`
//!
//! We represent the grammar and the input string as relations:
//! - **Terminals**: `(lhs: String, rhs: String)`
//! - **Non-Terminals**: `(lhs: String, rhs1: String, rhs2: String)`
//! - **Input**: `(pos: Int, char: String)`
//!
//! The parsing process iteratively builds a **Parse Table**: `(start: Int, length: Int, non_terminal: String)`
//!
//! Using relational algebra:
//! 1. We initialize the parse table by joining the `Input` with `Terminals`.
//! 2. We iteratively join the parse table with itself (finding adjacent parsed chunks)
//!    and with `Non-Terminals` to derive larger parsed chunks.
//! 3. We repeat until a fixpoint is reached (no new entries are added to the parse table).
//! 4. If the parse table contains an entry with `start = 0`, `length = input_length`,
//!    and `non_terminal = "S"` (start symbol), the string is accepted by the grammar!

use relvar_core::{
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational CYK Parser for Context-Free Grammars in Chomsky Normal Form.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::parser::CykParser;
/// // Placeholder example
/// ```
#[allow(dead_code)]
pub struct CykParser {
    /// Terminal rules: (lhs: String, rhs: String)
    pub terminals: Relation,
    /// Non-terminal rules: (lhs: String, rhs1: String, rhs2: String)
    pub non_terminals: Relation,
}

#[allow(dead_code)]
impl CykParser {
    /// Creates a new CYK parser with the given grammar rules.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::parser::CykParser;
    /// // Placeholder example
    /// ```
    pub fn new(terminals: Relation, non_terminals: Relation) -> Self {
        Self {
            terminals,
            non_terminals,
        }
    }

    /// Parses an input string (represented as a relation of characters)
    /// and returns the complete parse table.
    ///
    /// The input relation must have the schema: `(pos: Int, char: String)`
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{tuple, Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::parser::CykParser;
    ///
    /// // 1. Setup a simple grammar: S -> "a"
    /// let term_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("lhs", ScalarType::String)
    ///         .with_attribute("rhs", ScalarType::String)
    /// );
    /// let mut terminals = Relation::new(term_type);
    /// terminals.insert(tuple! { lhs: "S", rhs: "a" }).unwrap();
    ///
    /// let non_term_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("lhs", ScalarType::String)
    ///         .with_attribute("rhs1", ScalarType::String)
    ///         .with_attribute("rhs2", ScalarType::String)
    /// );
    /// let non_terminals = Relation::new(non_term_type);
    ///
    /// let parser = CykParser::new(terminals, non_terminals);
    ///
    /// // 2. Create input relation for string "a"
    /// let input_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("pos", ScalarType::Int)
    ///         .with_attribute("char", ScalarType::String)
    /// );
    /// let mut input = Relation::new(input_type);
    /// input.insert(tuple! { pos: 0i64, char: "a" }).unwrap();
    ///
    /// // 3. Parse and verify
    /// let parse_table = parser.parse(&input).unwrap();
    /// assert_eq!(parse_table.cardinality(), 1);
    /// ```
    pub fn parse(&self, input: &Relation) -> Result<Relation, DatabaseError> {
        let mut parse_table = self.initialize_parse_table(input)?;

        let mut new_entries_added = true;
        while new_entries_added {
            let prev_count = parse_table.cardinality();

            let new_entries = self.compute_new_entries(&parse_table)?;

            parse_table = parse_table
                .union(&new_entries)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            if parse_table.cardinality() == prev_count {
                new_entries_added = false;
            }
        }

        Ok(parse_table)
    }

    fn initialize_parse_table(&self, input: &Relation) -> Result<Relation, DatabaseError> {
        let init = input
            .clone()
            .rename(&[("char", "rhs")])
            .join(&self.terminals)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["pos", "lhs"])
            .rename(&[("pos", "start"), ("lhs", "non_terminal")]);

        init.extend("length", ScalarType::Int, |_: &Tuple| ScalarValue::Int(1))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn compute_new_entries(&self, parse_table: &Relation) -> Result<Relation, DatabaseError> {
        let left = parse_table
            .clone()
            .rename(&[("non_terminal", "rhs1"), ("length", "len1")]);

        let left_with_end = left
            .extend("start2", ScalarType::Int, |t: &Tuple| {
                let start = t.get_typed::<i64>("start").unwrap();
                let len = t.get_typed::<i64>("len1").unwrap();
                ScalarValue::Int(start + len)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let right = parse_table.clone().rename(&[
            ("start", "start2"),
            ("non_terminal", "rhs2"),
            ("length", "len2"),
        ]);

        let pairs = left_with_end
            .join(&right)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let matches = pairs
            .join(&self.non_terminals)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let new_entries = matches
            .extend("length", ScalarType::Int, |t: &Tuple| {
                let len1 = t.get_typed::<i64>("len1").unwrap();
                let len2 = t.get_typed::<i64>("len2").unwrap();
                ScalarValue::Int(len1 + len2)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["start", "length", "lhs"])
            .rename(&[("lhs", "non_terminal")]);

        Ok(new_entries)
    }

    /// Checks if a string is accepted by the grammar given its start symbol and length.
    /// It queries the `parse_table` produced by `parse` to see if there is an entry
    /// spanning the entire input length starting from position 0 for the given `start_symbol`.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar_core::{tuple, Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::parser::CykParser;
    ///
    /// // Build a parse table entry representing successful parse of "S" from pos 0 length 1
    /// let parse_table_type = RelationType::new(
    ///     TupleType::new()
    ///         .with_attribute("start", ScalarType::Int)
    ///         .with_attribute("length", ScalarType::Int)
    ///         .with_attribute("non_terminal", ScalarType::String)
    /// );
    /// let mut parse_table = Relation::new(parse_table_type);
    /// parse_table.insert(tuple! { start: 0i64, length: 1i64, non_terminal: "S" }).unwrap();
    ///
    /// let accepted = CykParser::is_accepted(&parse_table, "S", 1).unwrap();
    /// assert!(accepted);
    /// ```
    pub fn is_accepted(
        parse_table: &Relation,
        start_symbol: &str,
        input_length: i64,
    ) -> Result<bool, DatabaseError> {
        let accepted = parse_table.restrict(|t: &Tuple| {
            let start = t.get_typed::<i64>("start").unwrap();
            let length = t.get_typed::<i64>("length").unwrap();
            let nt = t.get_typed::<String>("non_terminal").unwrap();

            start == 0 && length == input_length && nt == start_symbol
        });

        Ok(accepted.cardinality() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{
        tuple,
        types::{RelationType, TupleType},
    };

    #[test]
    fn test_cyk_parser() {
        let term_type = RelationType::new(
            TupleType::new()
                .with_attribute("lhs", ScalarType::String)
                .with_attribute("rhs", ScalarType::String),
        );
        let mut terminals = Relation::new(term_type);
        terminals.insert(tuple! { lhs: "A", rhs: "a" }).unwrap();
        terminals.insert(tuple! { lhs: "B", rhs: "b" }).unwrap();

        let non_term_type = RelationType::new(
            TupleType::new()
                .with_attribute("lhs", ScalarType::String)
                .with_attribute("rhs1", ScalarType::String)
                .with_attribute("rhs2", ScalarType::String),
        );
        let mut non_terminals = Relation::new(non_term_type);
        non_terminals
            .insert(tuple! { lhs: "S", rhs1: "A", rhs2: "B" })
            .unwrap();

        let input_type = RelationType::new(
            TupleType::new()
                .with_attribute("pos", ScalarType::Int)
                .with_attribute("char", ScalarType::String),
        );
        let mut input = Relation::new(input_type);
        input.insert(tuple! { pos: 0i64, char: "a" }).unwrap();
        input.insert(tuple! { pos: 1i64, char: "b" }).unwrap();

        let parser = CykParser::new(terminals, non_terminals);
        let parse_table = parser.parse(&input).unwrap();

        assert!(CykParser::is_accepted(&parse_table, "S", 2).unwrap());
    }

    #[test]
    fn test_cyk_parser_complex() {
        let term_type = RelationType::new(
            TupleType::new()
                .with_attribute("lhs", ScalarType::String)
                .with_attribute("rhs", ScalarType::String),
        );
        let mut terminals = Relation::new(term_type);
        terminals.insert(tuple! { lhs: "A", rhs: "a" }).unwrap();
        terminals.insert(tuple! { lhs: "B", rhs: "b" }).unwrap();

        let non_term_type = RelationType::new(
            TupleType::new()
                .with_attribute("lhs", ScalarType::String)
                .with_attribute("rhs1", ScalarType::String)
                .with_attribute("rhs2", ScalarType::String),
        );
        let mut non_terminals = Relation::new(non_term_type);
        non_terminals
            .insert(tuple! { lhs: "S", rhs1: "A", rhs2: "B" })
            .unwrap();
        non_terminals
            .insert(tuple! { lhs: "S", rhs1: "S", rhs2: "S" })
            .unwrap();

        let parser = CykParser::new(terminals, non_terminals);

        let input_type = RelationType::new(
            TupleType::new()
                .with_attribute("pos", ScalarType::Int)
                .with_attribute("char", ScalarType::String),
        );
        let mut input = Relation::new(input_type);
        input.insert(tuple! { pos: 0i64, char: "a" }).unwrap();
        input.insert(tuple! { pos: 1i64, char: "b" }).unwrap();
        input.insert(tuple! { pos: 2i64, char: "a" }).unwrap();
        input.insert(tuple! { pos: 3i64, char: "b" }).unwrap();

        let parse_table = parser.parse(&input).unwrap();

        assert!(CykParser::is_accepted(&parse_table, "S", 4).unwrap());
        assert!(!CykParser::is_accepted(&parse_table, "S", 3).unwrap());
    }
}
