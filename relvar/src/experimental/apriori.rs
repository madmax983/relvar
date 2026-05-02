//! Relational Apriori Algorithm (Association Rule Learning)
//!
//! This module implements the Apriori algorithm for finding frequent itemsets
//! using purely relational algebra. It demonstrates how data mining algorithms
//! can be elegantly expressed as a sequence of relational joins, summarizations,
//! and restrictions.
//!
//! # Concept
//!
//! - **Transactions**: A relation `(tx_id: Int, item: String)`.
//! - **Support Counting**: Computed via `Summarize` and `Restrict`.
//! - **Candidate Generation**: Computed via Cross Joins (or Cartesian Products)
//!   and filtering.

use relvar_core::{algebra::Aggregation, error::DatabaseError, values::Relation};

/// The Apriori algorithm implementation using relational algebra.
///
/// # Examples
///
/// ```
/// use relvar_core::{values::Relation, types::{RelationType, TupleType, ScalarType}};
/// let heading = TupleType::new()
///     .with_attribute("tx_id", ScalarType::Int)
///     .with_attribute("item", ScalarType::String);
/// let transactions = Relation::new(RelationType::new(heading));
/// let apriori = relvar::experimental::apriori::RelationalApriori::new(transactions, 2);
/// ```
pub struct RelationalApriori {
    /// The transactions dataset.
    pub transactions: Relation,
    /// Minimum support threshold.
    pub min_support: i64,
}

impl RelationalApriori {
    /// Create a new `RelationalApriori`.
    pub fn new(transactions: Relation, min_support: i64) -> Self {
        Self {
            transactions,
            min_support,
        }
    }

    /// Finds all frequent 1-itemsets.
    /// Returns a relation with heading `(item: String, support: Int)`.
    pub fn find_frequent_1_itemsets(&self) -> Result<Relation, DatabaseError> {
        let counts = self
            .transactions
            .summarize(&["item"], &[Aggregation::count("support")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let min_sup = self.min_support;
        let frequent = counts.restrict(move |t| {
            if let Some(sup) = t.get_typed::<i64>("support") {
                sup >= min_sup
            } else {
                false
            }
        });

        Ok(frequent)
    }

    /// Finds all frequent 2-itemsets.
    /// Returns a relation with heading `(item1: String, item2: String, support: Int)`.
    pub fn find_frequent_2_itemsets(&self) -> Result<Relation, DatabaseError> {
        // 1. Get frequent 1-itemsets
        let l1 = self.find_frequent_1_itemsets()?;
        let l1_items = l1.project(&["item"]);

        // 2. Generate candidate 2-itemsets by joining L1 with itself.
        // We rename to avoid collision and filter to ensure item1 < item2.
        let l1_a = l1_items.rename(&[("item", "item1")]);
        let l1_b = l1_items.rename(&[("item", "item2")]);

        let candidates = l1_a.join(&l1_b)?.restrict(|t| {
            let i1 = t.get_typed::<String>("item1").unwrap();
            let i2 = t.get_typed::<String>("item2").unwrap();
            i1 < i2
        });

        // 3. Count support for each candidate pair
        // Join transactions with themselves: t1(tx_id, item1) ⨝ t2(tx_id, item2)
        let t1 = self.transactions.rename(&[("item", "item1")]);
        let t2 = self.transactions.rename(&[("item", "item2")]);

        let tx_pairs = t1.join(&t2)?.restrict(|t| {
            let i1 = t.get_typed::<String>("item1").unwrap();
            let i2 = t.get_typed::<String>("item2").unwrap();
            i1 < i2
        });

        // 4. Join candidates with tx_pairs to only count support for valid candidates
        let candidate_occurrences = candidates.join(&tx_pairs)?;

        // 5. Summarize by (item1, item2) to get the support count
        let counts = candidate_occurrences
            .summarize(&["item1", "item2"], &[Aggregation::count("support")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 6. Restrict to min_support
        let min_sup = self.min_support;
        let frequent_2 = counts.restrict(move |t| {
            if let Some(sup) = t.get_typed::<i64>("support") {
                sup >= min_sup
            } else {
                false
            }
        });

        Ok(frequent_2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_apriori() {
        let heading = TupleType::new()
            .with_attribute("tx_id", ScalarType::Int)
            .with_attribute("item", ScalarType::String);

        let mut transactions = Relation::new(RelationType::new(heading));

        // TX 1: Apple, Banana, Cherry
        transactions
            .insert(tuple! { tx_id: 1i64, item: "Apple".to_string() })
            .unwrap();
        transactions
            .insert(tuple! { tx_id: 1i64, item: "Banana".to_string() })
            .unwrap();
        transactions
            .insert(tuple! { tx_id: 1i64, item: "Cherry".to_string() })
            .unwrap();

        // TX 2: Apple, Banana
        transactions
            .insert(tuple! { tx_id: 2i64, item: "Apple".to_string() })
            .unwrap();
        transactions
            .insert(tuple! { tx_id: 2i64, item: "Banana".to_string() })
            .unwrap();

        // TX 3: Apple, Cherry
        transactions
            .insert(tuple! { tx_id: 3i64, item: "Apple".to_string() })
            .unwrap();
        transactions
            .insert(tuple! { tx_id: 3i64, item: "Cherry".to_string() })
            .unwrap();

        // TX 4: Banana
        transactions
            .insert(tuple! { tx_id: 4i64, item: "Banana".to_string() })
            .unwrap();

        // Apple: 3, Banana: 3, Cherry: 2
        // We want min_support = 2
        let apriori = RelationalApriori::new(transactions, 2);

        let l1 = apriori.find_frequent_1_itemsets().unwrap();
        assert_eq!(l1.cardinality(), 3); // All 3 have support >= 2

        let l2 = apriori.find_frequent_2_itemsets().unwrap();
        // Pairs with support >= 2:
        // Apple & Banana: TX 1, TX 2 -> Support 2 (Yes)
        // Apple & Cherry: TX 1, TX 3 -> Support 2 (Yes)
        // Banana & Cherry: TX 1 -> Support 1 (No)
        assert_eq!(l2.cardinality(), 2);
    }
}
