use super::ast::Query;
use crate::algebra::Aggregation;
use crate::constraints::ConstraintExpression;

impl Query {
    /// Creates a Scan query.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    ///
    /// let q = Query::scan("USERS");
    /// ```
    pub fn scan(table: impl Into<String>) -> Self {
        Query::Scan(table.into())
    }

    /// Wraps the query in a Restrict operation.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    /// use relvar_core::constraints::{ConstraintExpression, CmpOp, ValueOrRef};
    /// use relvar_core::values::ScalarValue;
    ///
    /// let condition = ConstraintExpression::Cmp {
    ///     left: "id".to_string(),
    ///     op: CmpOp::Eq,
    ///     right: ValueOrRef::Value(ScalarValue::Int(1))
    /// };
    /// let q = Query::scan("USERS").restrict(condition);
    /// ```
    pub fn restrict(self, predicate: ConstraintExpression) -> Self {
        Query::Restrict {
            input: Box::new(self),
            predicate,
        }
    }

    /// Wraps the query in a Project operation.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    ///
    /// let q = Query::scan("USERS").project(vec!["name", "email"]);
    /// ```
    pub fn project<S: Into<String>>(self, attributes: Vec<S>) -> Self {
        Query::Project {
            input: Box::new(self),
            attributes: attributes.into_iter().map(|s| s.into()).collect(),
        }
    }

    /// Wraps the query in a Rename operation.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    ///
    /// let q = Query::scan("USERS").rename(vec![("name", "full_name")]);
    /// ```
    pub fn rename<S1: Into<String>, S2: Into<String>>(self, mappings: Vec<(S1, S2)>) -> Self {
        Query::Rename {
            input: Box::new(self),
            mappings: mappings
                .into_iter()
                .map(|(a, b)| (a.into(), b.into()))
                .collect(),
        }
    }

    /// Joins this query with another query.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    ///
    /// let users = Query::scan("USERS");
    /// let orders = Query::scan("ORDERS");
    /// let joined = users.join(orders);
    /// ```
    pub fn join(self, right: Query) -> Self {
        Query::Join {
            left: Box::new(self),
            right: Box::new(right),
        }
    }

    /// Wraps the query in a Summarize operation.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::query::Query;
    /// use relvar_core::algebra::Aggregation;
    ///
    /// let q = Query::scan("USERS").summarize(vec!["department"], vec![Aggregation::count("emp_count")]);
    /// ```
    pub fn summarize<S: Into<String>>(
        self,
        group_by: Vec<S>,
        aggregations: Vec<Aggregation>,
    ) -> Self {
        Query::Summarize {
            input: Box::new(self),
            group_by: group_by.into_iter().map(|s| s.into()).collect(),
            aggregations,
        }
    }
}
