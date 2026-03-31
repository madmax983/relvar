use super::ast::{Query, QueryError};
use crate::database::Database;
use crate::storage_engine::StorageEngine;
use crate::values::Relation;

impl Query {
    /// Executes the query plan against the given relation source (e.g., Database).
    ///
    /// This method recursively evaluates the query tree.
    ///
    /// # Execution Process
    ///
    /// 1.  **Traverse**: The execution starts at the root node and recursively calls `execute` on children.
    /// 2.  **Scan**: Leaf nodes ([`Query::Scan`]) load base relations from the database.
    /// 3.  **Process**: Each internal node takes the resulting relation(s) from its children
    ///     and applies its relational algebra operator.
    /// 4.  **Optimize**: Some operators perform just-in-time optimization. For example,
    ///     [`Query::Restrict`] prepares its predicate for efficient evaluation.
    ///
    /// # Query Optimization
    ///
    /// The current implementation uses a simple "Volcano-style" iterator model (though materialized
    /// at each step). Optimization is currently limited to:
    ///
    /// - **Predicate Preparation**: `Restrict` pre-compiles `IN` lists into HashSets and
    ///   `LIKE` patterns into regex/matchers to avoid re-parsing per tuple.
    /// - **Push-down**: Not yet implemented. Future versions will push `Restrict` and `Project`
    ///   down the tree to minimize intermediate result sizes.
    ///
    /// # Errors
    ///
    /// Yields an error if:
    /// - A referenced relation does not exist.
    /// - A constraint expression is invalid (e.g., type mismatch).
    /// - An algebraic operation fails (e.g., joining incompatible types).
    /// # Examples
    ///
    /// ```
    /// use relvar_core::database::Database;
    /// use relvar_core::storage_engine::InMemoryEngine;
    /// use relvar_core::types::{RelationType, TupleType, ScalarType};
    /// use relvar_core::query::Query;
    /// use relvar_core::tuple;
    ///
    /// let mut db = Database::new(InMemoryEngine::new());
    /// let heading = TupleType::new().with_attribute("x", ScalarType::Int);
    /// db.create_relvar("TEST", RelationType::new(heading)).unwrap();
    /// db.insert("TEST", tuple!{ x: 1i64 }).unwrap();
    ///
    /// let q = Query::scan("TEST");
    /// let rel = q.execute(&db).unwrap();
    /// assert_eq!(rel.cardinality(), 1);
    /// ```
    pub fn execute<E: StorageEngine>(&self, db: &Database<E>) -> Result<Relation, QueryError> {
        match self {
            Query::Scan(table_name) => Ok(db.query(table_name)?),
            Query::Restrict { input, predicate } => {
                let relation = input.execute(db)?;

                // Pre-compute optimized structures (HashSet for IN, Vec<char> for LIKE)
                // This prevents O(N*M) behavior for large IN lists or LIKE patterns
                let prepared = predicate.prepare();

                // Manually restrict the relation to handle and propagate evaluation errors
                let mut eval_error = None;
                let result = relation.restrict_into(|tuple| {
                    match prepared.evaluate(tuple) {
                        Ok(res) => res,
                        Err(e) => {
                            if eval_error.is_none() {
                                eval_error = Some(e);
                            }
                            false // Treat as false to continue, but we'll error out anyway
                        }
                    }
                });

                if let Some(err) = eval_error {
                    return Err(QueryError::Constraint(err));
                }

                Ok(result)
            }
            Query::Project { input, attributes } => {
                let relation = input.execute(db)?;
                let attrs_ref: Vec<&str> = attributes.iter().map(|s| s.as_str()).collect();
                Ok(relation.project_into(&attrs_ref))
            }
            Query::Rename { input, mappings } => {
                let relation = input.execute(db)?;
                let mappings_ref: Vec<(&str, &str)> = mappings
                    .iter()
                    .map(|(a, b)| (a.as_str(), b.as_str()))
                    .collect();
                Ok(relation.rename(&mappings_ref))
            }
            Query::Join { left, right } => {
                let left_rel = left.execute(db)?;
                let right_rel = right.execute(db)?;
                Ok(left_rel.join(&right_rel)?)
            }
            Query::Summarize {
                input,
                group_by,
                aggregations,
            } => {
                let relation = input.execute(db)?;
                // aggregations is already Vec<Aggregation>, so no conversion needed
                let group_by_ref: Vec<&str> = group_by.iter().map(|s| s.as_str()).collect();
                Ok(relation
                    .summarize(&group_by_ref, &aggregations)
                    .map_err(|e| QueryError::Algebra(e.to_string()))?)
            }
        }
    }

    /// Yields a human-readable explanation of the query plan.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    ///
    /// let q = Query::scan("TEST").project(vec!["x"]);
    /// let explanation = q.explain();
    /// assert!(explanation.contains("Project"));
    /// assert!(explanation.contains("Scan"));
    /// ```
    pub fn explain(&self) -> String {
        self.explain_recursive(0)
    }

    fn explain_recursive(&self, depth: usize) -> String {
        let indent = "  ".repeat(depth);
        match self {
            Query::Scan(table) => format!("{}Scan({})", indent, table),
            Query::Restrict { input, predicate } => {
                format!(
                    "{}Restrict({:?})
{}",
                    indent,
                    predicate,
                    input.explain_recursive(depth + 1)
                )
            }
            Query::Project { input, attributes } => {
                format!(
                    "{}Project({:?})
{}",
                    indent,
                    attributes,
                    input.explain_recursive(depth + 1)
                )
            }
            Query::Rename { input, mappings } => {
                format!(
                    "{}Rename({:?})
{}",
                    indent,
                    mappings,
                    input.explain_recursive(depth + 1)
                )
            }
            Query::Join { left, right } => {
                format!(
                    "{}Join
{}
{}",
                    indent,
                    left.explain_recursive(depth + 1),
                    right.explain_recursive(depth + 1)
                )
            }
            Query::Summarize {
                input,
                group_by,
                aggregations,
            } => {
                format!(
                    "{}Summarize(group_by={:?}, aggs={:?})
{}",
                    indent,
                    group_by,
                    aggregations,
                    input.explain_recursive(depth + 1)
                )
            }
        }
    }
}
