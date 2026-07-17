//! Relational Apriori Algorithm (Market Basket Analysis)
//!
//! This module implements the Apriori algorithm for frequent itemset mining
//! using purely relational algebra. It discovers frequent individual items and
//! frequent pairs of items by performing relational summarization, restriction, and joins.
//!
//! # Example
//!
//! ```
//! use relvar_core::{tuple, Relation, RelationType, ScalarType, TupleType};
//! use relvar::experimental::apriori::Apriori;
//!
//! // 1. Setup transactions
//! let trans_type = RelationType::new(
//!     TupleType::new()
//!         .with_attribute("tid", ScalarType::Int)
//!         .with_attribute("item", ScalarType::String)
//! );
//! let mut transactions = Relation::new(trans_type);
//! transactions.insert(tuple! { tid: 1i64, item: "Apple" }).unwrap();
//! transactions.insert(tuple! { tid: 1i64, item: "Banana" }).unwrap();
//! transactions.insert(tuple! { tid: 2i64, item: "Apple" }).unwrap();
//! transactions.insert(tuple! { tid: 2i64, item: "Banana" }).unwrap();
//! transactions.insert(tuple! { tid: 3i64, item: "Banana" }).unwrap();
//! transactions.insert(tuple! { tid: 3i64, item: "Cherry" }).unwrap();
//!
//! // 2. Run Apriori
//! let apriori = Apriori::new(transactions);
//! let freq_pairs = apriori.frequent_2_itemsets(2).unwrap();
//!
//! // Apple and Banana appear together twice (tid 1 and 2)
//! assert_eq!(freq_pairs.cardinality(), 1);
//! ```

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    values::{Relation, Tuple},
};

/// A Relational Market Basket Analyzer.
pub struct Apriori {
    /// Transactions. Schema: (tid: Int, item: String)
    pub transactions: Relation,
}

impl Apriori {
    /// Creates a new Apriori analyzer.
    pub fn new(transactions: Relation) -> Self {
        Self { transactions }
    }

    /// Finds frequent individual items with support >= min_support.
    /// Returns Relation with Schema: (item: String, support: Int)
    pub fn frequent_1_itemsets(&self, min_support: i64) -> Result<Relation, DatabaseError> {
        let counts = self
            .transactions
            .summarize(&["item"], &[Aggregation::count("support")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(counts
            .restrict(move |t: &Tuple| t.get_typed::<i64>("support").unwrap_or(0) >= min_support))
    }

    /// Finds frequent pairs of items with support >= min_support.
    /// Returns Relation with Schema: (item1: String, item2: String, support: Int)
    pub fn frequent_2_itemsets(&self, min_support: i64) -> Result<Relation, DatabaseError> {
        // 1. Get frequent 1-itemsets
        let freq_1 = self.frequent_1_itemsets(min_support)?;
        let f1 = freq_1
            .clone()
            .project(&["item"])
            .rename(&[("item", "item1")]);
        let f2 = freq_1.project(&["item"]).rename(&[("item", "item2")]);

        // 2. Cross join to form candidates
        let candidates = f1
            .join(&f2)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Restrict to item1 < item2 to avoid duplicates and self-pairs
        let candidates = candidates.restrict(|t: &Tuple| {
            let i1 = t.get_typed::<String>("item1").unwrap();
            let i2 = t.get_typed::<String>("item2").unwrap();
            i1 < i2
        });

        // 4. Form transaction pairs: (tid, item1, item2)
        let t1 = self.transactions.clone().rename(&[("item", "item1")]);
        let t2 = self.transactions.clone().rename(&[("item", "item2")]);
        let trans_pairs = t1
            .join(&t2)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Join candidates with transaction pairs
        let valid_occurrences = candidates
            .join(&trans_pairs)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 6. Summarize to count support
        let pair_counts = valid_occurrences
            .summarize(&["item1", "item2"], &[Aggregation::count("support")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 7. Filter by min_support
        Ok(pair_counts
            .restrict(move |t: &Tuple| t.get_typed::<i64>("support").unwrap_or(0) >= min_support))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{
        tuple,
        types::{RelationType, ScalarType, TupleType},
    };

    #[test]
    fn test_apriori() {
        let trans_type = RelationType::new(
            TupleType::new()
                .with_attribute("tid", ScalarType::Int)
                .with_attribute("item", ScalarType::String),
        );
        let mut transactions = Relation::new(trans_type);

        // T1: A, B, C
        transactions
            .insert(tuple! { tid: 1i64, item: "A" })
            .unwrap();
        transactions
            .insert(tuple! { tid: 1i64, item: "B" })
            .unwrap();
        transactions
            .insert(tuple! { tid: 1i64, item: "C" })
            .unwrap();
        // T2: A, B
        transactions
            .insert(tuple! { tid: 2i64, item: "A" })
            .unwrap();
        transactions
            .insert(tuple! { tid: 2i64, item: "B" })
            .unwrap();
        // T3: A, C
        transactions
            .insert(tuple! { tid: 3i64, item: "A" })
            .unwrap();
        transactions
            .insert(tuple! { tid: 3i64, item: "C" })
            .unwrap();
        // T4: B, C
        transactions
            .insert(tuple! { tid: 4i64, item: "B" })
            .unwrap();
        transactions
            .insert(tuple! { tid: 4i64, item: "C" })
            .unwrap();

        let apriori = Apriori::new(transactions);

        let freq_1 = apriori.frequent_1_itemsets(3).unwrap();
        assert_eq!(freq_1.cardinality(), 3); // A(3), B(3), C(3)

        let freq_2 = apriori.frequent_2_itemsets(2).unwrap();
        // AB(2), AC(2), BC(2)
        assert_eq!(freq_2.cardinality(), 3);

        let freq_2_strict = apriori.frequent_2_itemsets(3).unwrap();
        // None have support 3
        assert_eq!(freq_2_strict.cardinality(), 0);
    }
}
