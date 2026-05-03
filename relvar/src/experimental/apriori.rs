//! Relational Apriori Algorithm (Frequent Itemset Mining)
//!
//! This module demonstrates how the Apriori algorithm for finding frequent itemsets
//! in transactional databases can be implemented using purely relational algebra.
//!
//! # Concept
//!
//! - **Transactions**: Relation `(tx_id: Int, item: String)`.
//!
//! We compute frequent itemsets in stages:
//! 1. **Frequent 1-Itemsets**: Summarize transactions grouping by `item`, counting `tx_id`.
//!    Restrict to those where count >= min_support. Project to just `(item)`.
//! 2. **Frequent 2-Itemsets**:
//!    - Generate candidate pairs by cross-joining frequent 1-itemsets with itself (renaming one side)
//!      and restricting to `item_a < item_b` to avoid duplicates and self-pairs.
//!    - Support counting: Join candidates with original transactions (on `item_a = item`), rename `tx_id` to `tx_id_a`.
//!      Join again with original transactions (on `item_b = item`), matching on `tx_id_a = tx_id_b`.
//!      Summarize to count matching transactions, and restrict by min_support.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,

    values::{Relation, Tuple},
};

/// A Relational Apriori Algorithm for Frequent Itemset Mining.
///
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType, tuple};
/// use relvar::experimental::apriori::FrequentItemsetMiner;
///
/// // Create a relation of transactions (tx_id: Int, item: String)
/// let tx_heading = TupleType::new()
///     .with_attribute("tx_id", ScalarType::Int)
///     .with_attribute("item", ScalarType::String);
/// let mut transactions = Relation::new(RelationType::new(tx_heading));
///
/// // Transaction 1: Apple, Banana
/// transactions.insert(tuple! { tx_id: 1i64, item: "Apple" }).unwrap();
/// transactions.insert(tuple! { tx_id: 1i64, item: "Banana" }).unwrap();
///
/// // Transaction 2: Apple, Banana, Cherry
/// transactions.insert(tuple! { tx_id: 2i64, item: "Apple" }).unwrap();
/// transactions.insert(tuple! { tx_id: 2i64, item: "Banana" }).unwrap();
/// transactions.insert(tuple! { tx_id: 2i64, item: "Cherry" }).unwrap();
///
/// // Transaction 3: Apple
/// transactions.insert(tuple! { tx_id: 3i64, item: "Apple" }).unwrap();
///
/// let miner = FrequentItemsetMiner::new(transactions);
/// let l1 = miner.frequent_1_itemsets(2).unwrap();
/// assert_eq!(l1.cardinality(), 2); // Apple and Banana
/// ```
pub struct FrequentItemsetMiner {
    /// The transactions relation. Schema: `(tx_id: Int, item: String)`
    pub transactions: Relation,
}

impl FrequentItemsetMiner {
    /// Creates a new FrequentItemsetMiner.
    ///
    /// # Arguments
    ///
    /// * `transactions` - A relation with schema `(tx_id: Int, item: String)`.
    pub fn new(transactions: Relation) -> Self {
        Self { transactions }
    }

    /// Finds frequent 1-itemsets (items that appear in at least `min_support` transactions).
    ///
    /// Returns a relation with schema `(item: String, support: Int)`.
    pub fn frequent_1_itemsets(&self, min_support: i64) -> Result<Relation, DatabaseError> {
        // Group by item and count tx_id
        let item_counts = self.transactions.summarize(
            &["item"],
            &[Aggregation::count("support")],
        ).map_err(|e| DatabaseError::TransactionError(e.to_string()))?;

        // Restrict to support >= min_support
        let frequent_items = item_counts.restrict(|t: &Tuple| {
            if let Some(support) = t.get_typed::<i64>("support") {
                support >= min_support
            } else {
                false
            }
        });

        Ok(frequent_items)
    }

    /// Finds frequent 2-itemsets (pairs of items that appear together in at least `min_support` transactions).
    ///
    /// Returns a relation with schema `(item_a: String, item_b: String, support: Int)`.
    pub fn frequent_2_itemsets(&self, min_support: i64) -> Result<Relation, DatabaseError> {
        // 1. Get frequent 1-itemsets
        let l1 = self.frequent_1_itemsets(min_support)?;
        let l1_items_only = l1.project(&["item"]);

        // 2. Generate Candidate 2-itemsets
        let l1_a = l1_items_only.rename(&[("item", "item_a")]);
        let l1_b = l1_items_only.rename(&[("item", "item_b")]);

        // Cross join to get all pairs
        let candidates = l1_a.join(&l1_b)?;

        // Restrict to item_a < item_b to avoid duplicates and self-pairs
        let valid_candidates = candidates.restrict(|t: &Tuple| {
            let a = t.get_typed::<String>("item_a").unwrap_or_default();
            let b = t.get_typed::<String>("item_b").unwrap_or_default();
            a < b
        });

        // 3. Count support for candidate pairs
        // tx_a: (tx_id_a, item_a)
        // let tx_a = self.transactions.rename(&[("tx_id", "tx_id_a"), ("item", "item_a")]);

        // Join candidates with tx_a to find transactions containing item_a
        // Result schema: (item_a, item_b, tx_id_a)
        // let cand_tx_a = valid_candidates.join(&tx_a)?;

        // tx_b: (tx_id_b, item_b)
        // Wait, to find transactions containing both item_a and item_b, we need the SAME transaction id.
        // So let's just use tx_id instead of renaming it differently.
        let tx_a_same = self.transactions.rename(&[("item", "item_a")]);
        let tx_b_same = self.transactions.rename(&[("item", "item_b")]);

        // cand_with_tx_a: (item_a, item_b, tx_id)
        let cand_with_tx_a = valid_candidates.join(&tx_a_same)?;

        // cand_with_tx_a_and_b: (item_a, item_b, tx_id)
        let cand_with_tx_a_and_b = cand_with_tx_a.join(&tx_b_same)?;

        // 4. Summarize by (item_a, item_b) counting tx_id
        let pair_counts = cand_with_tx_a_and_b.summarize(
            &["item_a", "item_b"],
            &[Aggregation::count("support")],
        ).map_err(|e| DatabaseError::TransactionError(e.to_string()))?;

        // 5. Restrict to support >= min_support
        let frequent_pairs = pair_counts.restrict(|t: &Tuple| {
            if let Some(support) = t.get_typed::<i64>("support") {
                support >= min_support
            } else {
                false
            }
        });

        Ok(frequent_pairs)
    }
}

#[cfg(test)]
mod tests {
    use relvar_core::types::{RelationType, ScalarType, TupleType};
    use super::*;
    use relvar_core::tuple;

    fn setup_transactions() -> Relation {
        let tx_heading = TupleType::new()
            .with_attribute("tx_id", ScalarType::Int)
            .with_attribute("item", ScalarType::String);
        let mut transactions = Relation::new(RelationType::new(tx_heading));

        // T1: Apple, Banana, Cherry
        transactions.insert(tuple! { tx_id: 1i64, item: "Apple" }).unwrap();
        transactions.insert(tuple! { tx_id: 1i64, item: "Banana" }).unwrap();
        transactions.insert(tuple! { tx_id: 1i64, item: "Cherry" }).unwrap();

        // T2: Apple, Banana
        transactions.insert(tuple! { tx_id: 2i64, item: "Apple" }).unwrap();
        transactions.insert(tuple! { tx_id: 2i64, item: "Banana" }).unwrap();

        // T3: Apple, Date
        transactions.insert(tuple! { tx_id: 3i64, item: "Apple" }).unwrap();
        transactions.insert(tuple! { tx_id: 3i64, item: "Date" }).unwrap();

        // T4: Banana, Cherry
        transactions.insert(tuple! { tx_id: 4i64, item: "Banana" }).unwrap();
        transactions.insert(tuple! { tx_id: 4i64, item: "Cherry" }).unwrap();

        transactions
    }

    #[test]
    fn test_frequent_1_itemsets() {
        let transactions = setup_transactions();
        let miner = FrequentItemsetMiner::new(transactions);

        let freq1 = miner.frequent_1_itemsets(2).unwrap();

        // Apple (3), Banana (3), Cherry (2), Date (1 - filtered)
        assert_eq!(freq1.cardinality(), 3);

        // Verify counts
        for tuple in freq1.tuples() {
            let item = tuple.get_typed::<String>("item").unwrap();
            let support = tuple.get_typed::<i64>("support").unwrap();
            match item.as_str() {
                "Apple" => assert_eq!(support, 3),
                "Banana" => assert_eq!(support, 3),
                "Cherry" => assert_eq!(support, 2),
                _ => panic!("Unexpected item: {}", item),
            }
        }
    }

    #[test]
    fn test_frequent_2_itemsets() {
        let transactions = setup_transactions();
        let miner = FrequentItemsetMiner::new(transactions);

        let freq2 = miner.frequent_2_itemsets(2).unwrap();

        // Apple & Banana (2), Banana & Cherry (2)
        // Apple & Cherry (1 - filtered)
        assert_eq!(freq2.cardinality(), 2);

        for tuple in freq2.tuples() {
            let item_a = tuple.get_typed::<String>("item_a").unwrap();
            let item_b = tuple.get_typed::<String>("item_b").unwrap();
            let support = tuple.get_typed::<i64>("support").unwrap();
            assert_eq!(support, 2);

            if (item_a == "Apple" && item_b == "Banana") || (item_a == "Banana" && item_b == "Cherry") {



            } else {
                panic!("Unexpected pair: {}, {}", item_a, item_b);
            }
        }
    }
}
