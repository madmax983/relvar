//! Relational Expert System (Forward Chaining Inference Engine)
//!
//! This module demonstrates how an Expert System (or Business Rules Engine)
//! can be implemented using purely relational algebra operations.
//!
//! # Concept
//!
//! We represent facts and rules as relations:
//! - **Facts**: `(fact: String)`
//! - **Rule Conditions**: `(rule_id: String, condition: String)`
//! - **Rule Conclusions**: `(rule_id: String, conclusion: String)`
//!
//! Forward chaining is executed iteratively:
//! 1. Find which conditions are satisfied by joining `Rule Conditions` with `Facts`.
//! 2. Summarize to count satisfied conditions per rule.
//! 3. Compare with the total number of conditions per rule.
//! 4. For rules where all conditions are met, extract their conclusions.
//! 5. Add new conclusions to the `Facts` relation.
//! 6. Repeat until no new facts are derived (fixpoint).

use relvar_core::{algebra::Aggregation, error::DatabaseError, values::Relation};

/// A relational Expert System using forward chaining.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::expert_system::ExpertSystem;
/// // Note: This is a placeholder example
/// ```
pub struct ExpertSystem {
    /// Initial facts. Schema: (fact: String)
    pub facts: Relation,
    /// Conditions for rules. Schema: (rule_id: String, condition: String)
    pub rule_conditions: Relation,
    /// Conclusions for rules. Schema: (rule_id: String, conclusion: String)
    pub rule_conclusions: Relation,
}

impl ExpertSystem {
    /// Creates a new Expert System.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::expert_system::ExpertSystem;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(facts: Relation, rule_conditions: Relation, rule_conclusions: Relation) -> Self {
        Self {
            facts,
            rule_conditions,
            rule_conclusions,
        }
    }

    /// Evaluates the rules until a fixpoint is reached (no new facts can be derived).
    /// Returns the complete set of facts.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::expert_system::ExpertSystem;
    /// // Note: This is a placeholder example
    /// ```
    pub fn infer(&self) -> Result<Relation, DatabaseError> {
        let mut current_facts = self.facts.clone();

        if self.rule_conditions.cardinality() == 0 {
            return Ok(current_facts);
        }

        // Pre-compute the total number of conditions per rule
        let total_conditions = self
            .rule_conditions
            .summarize(&["rule_id"], &[Aggregation::count("total_conds")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        loop {
            // Rename 'fact' to 'condition' to join with rule_conditions
            let facts_as_conds = current_facts.rename(&[("fact", "condition")]);

            // Which conditions are satisfied?
            let satisfied_conditions = self.rule_conditions.join(&facts_as_conds)?;

            // If no conditions are satisfied at all, we can't trigger anything new
            if satisfied_conditions.cardinality() == 0 {
                break;
            }

            // Count satisfied conditions per rule
            let satisfied_counts = satisfied_conditions
                .summarize(&["rule_id"], &[Aggregation::count("satisfied_conds")])
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Join totals with satisfied counts
            let rule_stats = total_conditions.join(&satisfied_counts)?;

            // Filter rules where satisfied_conds == total_conds
            let triggered_rules = rule_stats
                .restrict(|t| {
                    let total = t.get_typed::<i64>("total_conds").unwrap_or(0);
                    let satisfied = t.get_typed::<i64>("satisfied_conds").unwrap_or(0);
                    total == satisfied
                })
                .project(&["rule_id"]);

            if triggered_rules.cardinality() == 0 {
                break;
            }

            // Get conclusions for triggered rules
            let new_conclusions = triggered_rules.join(&self.rule_conclusions)?;

            // Project and rename to match facts schema
            let new_facts = new_conclusions
                .project(&["conclusion"])
                .rename(&[("conclusion", "fact")]);

            // Union with current facts
            let next_facts = current_facts
                .union(&new_facts)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Check for fixpoint (no new facts added)
            let diff = next_facts
                .difference(&current_facts)
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            if diff.is_empty() {
                break;
            }

            current_facts = next_facts;
        }

        Ok(current_facts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_socrates_mortal() {
        let fact_heading = TupleType::new().with_attribute("fact", ScalarType::String);
        let mut facts = Relation::new(RelationType::new(fact_heading));
        facts.insert(tuple! { fact: "Socrates is a man" }).unwrap();

        let cond_heading = TupleType::new()
            .with_attribute("rule_id", ScalarType::String)
            .with_attribute("condition", ScalarType::String);
        let mut rule_conditions = Relation::new(RelationType::new(cond_heading));
        rule_conditions
            .insert(tuple! { rule_id: "rule1", condition: "Socrates is a man" })
            .unwrap();

        let conc_heading = TupleType::new()
            .with_attribute("rule_id", ScalarType::String)
            .with_attribute("conclusion", ScalarType::String);
        let mut rule_conclusions = Relation::new(RelationType::new(conc_heading));
        rule_conclusions
            .insert(tuple! { rule_id: "rule1", conclusion: "Socrates is mortal" })
            .unwrap();

        let expert_system = ExpertSystem::new(facts, rule_conditions, rule_conclusions);
        let final_facts = expert_system.infer().unwrap();

        assert_eq!(final_facts.cardinality(), 2);

        let has_mortal = final_facts
            .tuples()
            .any(|t| t.get_typed::<String>("fact").unwrap() == "Socrates is mortal");
        assert!(has_mortal);
    }

    #[test]
    fn test_multiple_conditions() {
        let fact_heading = TupleType::new().with_attribute("fact", ScalarType::String);
        let mut facts = Relation::new(RelationType::new(fact_heading));
        facts.insert(tuple! { fact: "Has fur" }).unwrap();
        facts.insert(tuple! { fact: "Barks" }).unwrap();

        let cond_heading = TupleType::new()
            .with_attribute("rule_id", ScalarType::String)
            .with_attribute("condition", ScalarType::String);
        let mut rule_conditions = Relation::new(RelationType::new(cond_heading));
        // Rule: Dog
        rule_conditions
            .insert(tuple! { rule_id: "dog_rule", condition: "Has fur" })
            .unwrap();
        rule_conditions
            .insert(tuple! { rule_id: "dog_rule", condition: "Barks" })
            .unwrap();
        // Rule: Cat
        rule_conditions
            .insert(tuple! { rule_id: "cat_rule", condition: "Has fur" })
            .unwrap();
        rule_conditions
            .insert(tuple! { rule_id: "cat_rule", condition: "Meows" })
            .unwrap();

        let conc_heading = TupleType::new()
            .with_attribute("rule_id", ScalarType::String)
            .with_attribute("conclusion", ScalarType::String);
        let mut rule_conclusions = Relation::new(RelationType::new(conc_heading));
        rule_conclusions
            .insert(tuple! { rule_id: "dog_rule", conclusion: "Is a Dog" })
            .unwrap();
        rule_conclusions
            .insert(tuple! { rule_id: "cat_rule", conclusion: "Is a Cat" })
            .unwrap();

        let expert_system = ExpertSystem::new(facts, rule_conditions, rule_conclusions);
        let final_facts = expert_system.infer().unwrap();

        // Initial facts: 2. Derived facts: Is a Dog (1). Total: 3.
        assert_eq!(final_facts.cardinality(), 3);

        let is_dog = final_facts
            .tuples()
            .any(|t| t.get_typed::<String>("fact").unwrap() == "Is a Dog");
        assert!(is_dog);

        let is_cat = final_facts
            .tuples()
            .any(|t| t.get_typed::<String>("fact").unwrap() == "Is a Cat");
        assert!(!is_cat);
    }

    #[test]
    fn test_chained_inference() {
        let fact_heading = TupleType::new().with_attribute("fact", ScalarType::String);
        let mut facts = Relation::new(RelationType::new(fact_heading));
        facts.insert(tuple! { fact: "A" }).unwrap();

        let cond_heading = TupleType::new()
            .with_attribute("rule_id", ScalarType::String)
            .with_attribute("condition", ScalarType::String);
        let mut rule_conditions = Relation::new(RelationType::new(cond_heading));
        rule_conditions
            .insert(tuple! { rule_id: "r1", condition: "A" })
            .unwrap();
        rule_conditions
            .insert(tuple! { rule_id: "r2", condition: "B" })
            .unwrap();
        rule_conditions
            .insert(tuple! { rule_id: "r3", condition: "C" })
            .unwrap();

        let conc_heading = TupleType::new()
            .with_attribute("rule_id", ScalarType::String)
            .with_attribute("conclusion", ScalarType::String);
        let mut rule_conclusions = Relation::new(RelationType::new(conc_heading));
        rule_conclusions
            .insert(tuple! { rule_id: "r1", conclusion: "B" })
            .unwrap();
        rule_conclusions
            .insert(tuple! { rule_id: "r2", conclusion: "C" })
            .unwrap();
        rule_conclusions
            .insert(tuple! { rule_id: "r3", conclusion: "D" })
            .unwrap();

        let expert_system = ExpertSystem::new(facts, rule_conditions, rule_conclusions);
        let final_facts = expert_system.infer().unwrap();

        // A -> B -> C -> D
        assert_eq!(final_facts.cardinality(), 4);
        assert!(
            final_facts
                .tuples()
                .any(|t| t.get_typed::<String>("fact").unwrap() == "D")
        );
    }
}
