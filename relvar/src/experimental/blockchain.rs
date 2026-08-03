#![allow(dead_code)]
//! Relational Blockchain Simulation
//!
//! This module demonstrates how a simple blockchain ledger can be modeled and
//! validated using purely relational algebra. Blocks, transactions, and the
//! resulting state (balances) are all represented as relations.
//!
//! # Concept
//!
//! - **Transactions**: Relation `(tx_id: String, from: String, to: String, amount: Int, block_id: Int)`.
//! - **Blocks**: Relation `(block_id: Int, prev_hash: String, hash: String)`.
//! - **Ledger State**: Derived purely from aggregating the transactions relation.
//!
//! We can validate the integrity of the chain (hashes link correctly) and
//! ensure no double-spending (balances never drop below 0) using Relational
//! operations like Join, Summarize, and Restrict.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Blockchain.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::blockchain::Blockchain;
/// // Note: This is a placeholder example
/// ```
pub struct Blockchain {
    /// The blocks in the chain. Schema: (block_id: Int, prev_hash: String, hash: String)
    pub blocks: Relation,
    /// The transactions in the chain. Schema: (tx_id: String, from: String, to: String, amount: Int, block_id: Int)
    pub transactions: Relation,
}

impl Blockchain {
    /// Creates a new Blockchain from relations.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::blockchain::Blockchain;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(blocks: Relation, transactions: Relation) -> Self {
        Self {
            blocks,
            transactions,
        }
    }

    /// Computes the final balances for all accounts by aggregating transactions.
    ///
    /// This is done by:
    /// 1. Projecting and renaming 'from' to 'account' and negating 'amount'.
    /// 2. Projecting and renaming 'to' to 'account' and keeping 'amount'.
    /// 3. Unioning both sets.
    /// 4. Summarizing by 'account' and summing the amounts.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::blockchain::Blockchain;
    /// // Note: This is a placeholder example
    /// ```
    pub fn compute_balances(&self) -> Result<Relation, DatabaseError> {
        // Debits (from)
        let debits = self
            .transactions
            .project(&["tx_id", "from", "amount"])
            .extend("net_amount", ScalarType::Int, |t| {
                let amount = t.get_typed::<i64>("amount").unwrap();
                // Special case: "Mint" (or system) has infinite balance, but we can track it.
                // To avoid DoS from large values, use checked operations or saturating if needed.
                // However, for pure relational algebra demo, simple negation is fine here unless sizes are huge.
                // Security memory constraint: strictly use `saturating_*` or `checked_*` arithmetic.
                ScalarValue::Int(amount.saturating_neg())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["tx_id", "from", "net_amount"])
            .rename(&[("from", "account"), ("net_amount", "amount")]);

        // Credits (to)
        // Keep tx_id to ensure set-based union does not deduplicate identical amount transactions
        let credits = self
            .transactions
            .project(&["tx_id", "to", "amount"])
            .rename(&[("to", "account")]);

        // Combine
        let all_movements = debits
            .union(&credits)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Summarize
        all_movements
            .summarize(&["account"], &[Aggregation::sum("balance", "amount")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    /// Validates the chain. Returns true if valid, false if invalid.
    ///
    /// Checks:
    /// 1. Hashes link up: Every block's prev_hash matches the previous block's hash.
    ///    (Except genesis block).
    /// 2. No negative balances: The final balances of all accounts (except "Mint") must be >= 0.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::blockchain::Blockchain;
    /// // Note: This is a placeholder example
    /// ```
    pub fn is_valid(&self) -> Result<bool, DatabaseError> {
        // 1. Check balances
        let balances = self.compute_balances()?;

        let negative_balances = balances.restrict(|t| {
            let account = t.get_typed::<String>("account").unwrap();
            let balance = t.get_typed::<i64>("balance").unwrap();
            account != "Mint" && balance < 0
        });

        if negative_balances.cardinality() > 0 {
            return Ok(false);
        }

        // 2. Check block links
        // We self-join blocks: Current(block_id, prev_hash) with Prev(block_id, hash)
        // where Current.block_id = Prev.block_id + 1

        let current_blocks = self.blocks.rename(&[
            ("block_id", "curr_id"),
            ("prev_hash", "curr_prev_hash"),
            ("hash", "curr_hash"),
        ]);

        let prev_blocks = self.blocks.rename(&[
            ("block_id", "prev_id"),
            ("prev_hash", "p_prev_hash"), // unused
            ("hash", "p_hash"),
        ]);

        let joined = current_blocks.theta_join(&prev_blocks, |curr, prev| {
            let curr_id = curr.get_typed::<i64>("curr_id").unwrap();
            let prev_id = prev.get_typed::<i64>("prev_id").unwrap();
            curr_id == prev_id.saturating_add(1)
        });

        // Ensure chain is contiguous
        let expected_links = self.blocks.cardinality().saturating_sub(1);
        if joined.cardinality() != expected_links {
            return Ok(false);
        }

        // Restrict to where the hashes DO NOT match
        let invalid_links = joined.restrict(|t| {
            let curr_prev_hash = t.get_typed::<String>("curr_prev_hash").unwrap();
            let p_hash = t.get_typed::<String>("p_hash").unwrap();
            curr_prev_hash != p_hash
        });

        if invalid_links.cardinality() > 0 {
            return Ok(false);
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_valid_blockchain() {
        let block_heading = TupleType::new()
            .with_attribute("block_id", ScalarType::Int)
            .with_attribute("prev_hash", ScalarType::String)
            .with_attribute("hash", ScalarType::String);
        let mut blocks = Relation::new(RelationType::new(block_heading));
        blocks
            .insert(tuple! { block_id: 1i64, prev_hash: "0000", hash: "1111" })
            .unwrap();
        blocks
            .insert(tuple! { block_id: 2i64, prev_hash: "1111", hash: "2222" })
            .unwrap();

        let tx_heading = TupleType::new()
            .with_attribute("tx_id", ScalarType::String)
            .with_attribute("from", ScalarType::String)
            .with_attribute("to", ScalarType::String)
            .with_attribute("amount", ScalarType::Int)
            .with_attribute("block_id", ScalarType::Int);
        let mut txs = Relation::new(RelationType::new(tx_heading));
        txs.insert(
            tuple! { tx_id: "tx1", from: "Mint", to: "Alice", amount: 100i64, block_id: 1i64 },
        )
        .unwrap();
        txs.insert(
            tuple! { tx_id: "tx2", from: "Alice", to: "Bob", amount: 30i64, block_id: 2i64 },
        )
        .unwrap();

        let bc = Blockchain::new(blocks, txs);

        let balances = bc.compute_balances().unwrap();

        let alice_bal = balances
            .tuples()
            .find(|t| t.get_typed::<String>("account").unwrap() == "Alice")
            .unwrap()
            .get_typed::<i64>("balance")
            .unwrap();
        assert_eq!(alice_bal, 70);

        let bob_bal = balances
            .tuples()
            .find(|t| t.get_typed::<String>("account").unwrap() == "Bob")
            .unwrap()
            .get_typed::<i64>("balance")
            .unwrap();
        assert_eq!(bob_bal, 30);

        assert!(bc.is_valid().unwrap());
    }

    #[test]
    fn test_invalid_hash_link() {
        let block_heading = TupleType::new()
            .with_attribute("block_id", ScalarType::Int)
            .with_attribute("prev_hash", ScalarType::String)
            .with_attribute("hash", ScalarType::String);
        let mut blocks = Relation::new(RelationType::new(block_heading));
        blocks
            .insert(tuple! { block_id: 1i64, prev_hash: "0000", hash: "1111" })
            .unwrap();
        blocks
            .insert(tuple! { block_id: 2i64, prev_hash: "WRONG", hash: "2222" })
            .unwrap();

        let tx_heading = TupleType::new()
            .with_attribute("tx_id", ScalarType::String)
            .with_attribute("from", ScalarType::String)
            .with_attribute("to", ScalarType::String)
            .with_attribute("amount", ScalarType::Int)
            .with_attribute("block_id", ScalarType::Int);
        let txs = Relation::new(RelationType::new(tx_heading));

        let bc = Blockchain::new(blocks, txs);
        assert!(!bc.is_valid().unwrap());
    }

    #[test]
    fn test_double_spend_negative_balance() {
        let block_heading = TupleType::new()
            .with_attribute("block_id", ScalarType::Int)
            .with_attribute("prev_hash", ScalarType::String)
            .with_attribute("hash", ScalarType::String);
        let mut blocks = Relation::new(RelationType::new(block_heading));
        blocks
            .insert(tuple! { block_id: 1i64, prev_hash: "0000", hash: "1111" })
            .unwrap();

        let tx_heading = TupleType::new()
            .with_attribute("tx_id", ScalarType::String)
            .with_attribute("from", ScalarType::String)
            .with_attribute("to", ScalarType::String)
            .with_attribute("amount", ScalarType::Int)
            .with_attribute("block_id", ScalarType::Int);
        let mut txs = Relation::new(RelationType::new(tx_heading));
        txs.insert(
            tuple! { tx_id: "tx1", from: "Mint", to: "Alice", amount: 10i64, block_id: 1i64 },
        )
        .unwrap();
        txs.insert(
            tuple! { tx_id: "tx2", from: "Alice", to: "Bob", amount: 20i64, block_id: 1i64 },
        )
        .unwrap();

        let bc = Blockchain::new(blocks, txs);
        assert!(!bc.is_valid().unwrap()); // Alice went to -10
    }
}
