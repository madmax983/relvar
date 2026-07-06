//! Relational Genetic Algorithm
//!
//! This module demonstrates how a Genetic Algorithm can be implemented using purely
//! relational algebra operations. It represents a population as a relation, computes
//! fitness using `Extend`, selects the top individuals by implementing a relational rank
//! using `Theta-Join` and `Summarize`, and reproduces via `Join` and `Extend`.
//!
//! # Concept
//!
//! We represent the population as a relation of individuals: `(id: Int, gene_a: Int, gene_b: Int)`.
//!
//! To compute a generation:
//! 1. Evaluate Fitness: `Extend` to calculate `fitness = f(gene_a, gene_b)`.
//! 2. Selection: Find the top N individuals using a theta-join on fitness, grouping, and restricting by rank.
//! 3. Reproduction: Cross-join (or pair up) selected individuals and `Extend` to create offspring, applying crossover and mutations.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A relational Genetic Algorithm.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::genetic_algorithm::GeneticAlgorithm;
/// // Note: This is a placeholder example
/// ```
pub struct GeneticAlgorithm {
    /// The current population. Schema: (id: Int, gene_a: Int, gene_b: Int)
    pub population: Relation,
    /// The target sum for fitness calculation.
    pub target_sum: i64,
}

impl GeneticAlgorithm {
    /// Creates a new GeneticAlgorithm.
    ///
    /// # Arguments
    ///
    /// * `population` - A relation representing the initial population.
    /// * `target_sum` - The ideal sum of `gene_a` + `gene_b` for maximum fitness.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::genetic_algorithm::GeneticAlgorithm;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(population: Relation, target_sum: i64) -> Self {
        Self {
            population,
            target_sum,
        }
    }

    /// Computes the next generation.
    /// Returns the new population relation and the highest fitness found in the current generation.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::genetic_algorithm::GeneticAlgorithm;
    /// // Note: This is a placeholder example
    /// ```
    pub fn next_generation(&self, elite_count: usize) -> Result<(Relation, i64), DatabaseError> {
        if self.population.cardinality() == 0 {
            return Ok((self.population.clone(), 0));
        }

        // 1. Evaluate fitness
        let (evaluated, max_f) = self.evaluate_fitness()?;
        let parents = self.select_parents(&evaluated, elite_count)?;
        let next_pop = self.reproduce(&parents)?;

        Ok((next_pop, max_f))
    }

    fn evaluate_fitness(&self) -> Result<(Relation, i64), DatabaseError> {
        // fitness = -abs((gene_a + gene_b) - target_sum)
        let target = self.target_sum;
        let evaluated = self
            .population
            .extend("fitness", ScalarType::Int, move |t| {
                let a = t.get_typed::<i64>("gene_a").unwrap();
                let b = t.get_typed::<i64>("gene_b").unwrap();
                let sum = a.saturating_add(b);
                let diff = sum.abs_diff(target) as i64;
                ScalarValue::Int(-diff) // Negative distance so higher is better
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Find max fitness for reporting
        let max_fitness_rel = evaluated
            .summarize(
                &[],
                &[Aggregation::max("max_f", "fitness", ScalarType::Int)],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let max_f = max_fitness_rel
            .tuples()
            .next()
            .unwrap()
            .get_typed::<i64>("max_f")
            .unwrap();

        Ok((evaluated, max_f))
    }

    fn select_parents(
        &self,
        evaluated: &Relation,
        elite_count: usize,
    ) -> Result<Relation, DatabaseError> {
        // Selection: Top N using pure relational rank
        // We join `evaluated` with itself on `fitness < other_fitness` to count how many are better.
        let eval_p1 = evaluated.rename(&[
            ("id", "id1"),
            ("gene_a", "a1"),
            ("gene_b", "b1"),
            ("fitness", "f1"),
        ]);
        let eval_p2 = evaluated.rename(&[
            ("id", "id2"),
            ("gene_a", "a2"),
            ("gene_b", "b2"),
            ("fitness", "f2"),
        ]);

        let joined = eval_p1.theta_join(&eval_p2, |t1, t2| {
            let f1 = t1.get_typed::<i64>("f1").unwrap();
            let f2 = t2.get_typed::<i64>("f2").unwrap();
            f1 < f2
                || (f1 == f2
                    && t1.get_typed::<i64>("id1").unwrap() < t2.get_typed::<i64>("id2").unwrap()) // Tie break by ID
        });

        // Count how many `f2` are strictly better than `f1`
        let ranks = joined
            .summarize(&["id1"], &[Aggregation::count("rank")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Left outer join simulation for the best individual(s) which have no one better (rank 0 implicitly)
        let ids_with_rank = ranks.project(&["id1"]);
        let all_ids = eval_p1.project(&["id1"]);
        let top_ids = all_ids.difference(&ids_with_rank).unwrap(); // These are rank 0
        let top_with_zero = top_ids
            .extend("rank", ScalarType::Int, |_| ScalarValue::Int(0))
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let all_ranks = ranks
            .union(&top_with_zero)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let elite_count_i64 = elite_count as i64;
        let selected_ids = all_ranks
            .restrict(|t| {
                let rank = t.get_typed::<i64>("rank").unwrap();
                rank < elite_count_i64
            })
            .project(&["id1"])
            .rename(&[("id1", "id")]);

        let parents = evaluated
            .join(&selected_ids)?
            .project(&["id", "gene_a", "gene_b"]);

        Ok(parents)
    }

    fn reproduce(&self, parents: &Relation) -> Result<Relation, DatabaseError> {
        // Reproduction
        // Cross join parents to create pairs
        let p1 = parents.rename(&[("id", "id1"), ("gene_a", "a1"), ("gene_b", "b1")]);
        let p2 = parents.rename(&[("id", "id2"), ("gene_a", "a2"), ("gene_b", "b2")]);

        let pairs = p1.join(&p2)?;

        // Filter out self-pairs to get diverse parents if possible
        // If elite_count is 1, we must self-pair, otherwise we filter
        let valid_pairs = if parents.cardinality() > 1 {
            pairs.restrict(|t| {
                t.get_typed::<i64>("id1").unwrap() != t.get_typed::<i64>("id2").unwrap()
            })
        } else {
            pairs.clone()
        };

        // Create offspring by averaging genes (a simple crossover) and adding a slight mutation
        // Relational algebra lacks a global stateful generator, so we deterministically derive IDs
        let offspring = valid_pairs
            .extend("new_id", ScalarType::Int, |t| {
                ScalarValue::Int(
                    t.get_typed::<i64>("id1")
                        .unwrap()
                        .saturating_mul(100)
                        .saturating_add(t.get_typed::<i64>("id2").unwrap()),
                )
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_gene_a", ScalarType::Int, |t| {
                let avg =
                    (t.get_typed::<i64>("a1").unwrap() + t.get_typed::<i64>("a2").unwrap()) / 2;
                let mutation = if t.get_typed::<i64>("id1").unwrap() % 2 == 0 {
                    1
                } else {
                    -1
                };
                ScalarValue::Int(avg + mutation)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("new_gene_b", ScalarType::Int, |t| {
                let avg =
                    (t.get_typed::<i64>("b1").unwrap() + t.get_typed::<i64>("b2").unwrap()) / 2;
                let mutation = if t.get_typed::<i64>("id2").unwrap() % 3 == 0 {
                    -1
                } else {
                    1
                };
                ScalarValue::Int(avg + mutation)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["new_id", "new_gene_a", "new_gene_b"])
            .rename(&[
                ("new_id", "id"),
                ("new_gene_a", "gene_a"),
                ("new_gene_b", "gene_b"),
            ]);

        // Keep the best parents (elitism) and add offspring
        let next_pop = parents
            .union(&offspring)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(next_pop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_genetic_algorithm() {
        let heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("gene_a", ScalarType::Int)
            .with_attribute("gene_b", ScalarType::Int);
        let mut population = Relation::new(RelationType::new(heading));

        // Insert initial population
        population
            .insert(tuple! { id: 1i64, gene_a: 10i64, gene_b: 20i64 })
            .unwrap(); // sum 30
        population
            .insert(tuple! { id: 2i64, gene_a: 5i64, gene_b: 5i64 })
            .unwrap(); // sum 10
        population
            .insert(tuple! { id: 3i64, gene_a: 40i64, gene_b: 40i64 })
            .unwrap(); // sum 80
        population
            .insert(tuple! { id: 4i64, gene_a: 20i64, gene_b: 30i64 })
            .unwrap(); // sum 50

        // Target sum is 50. ID 4 is the best (0 diff).
        let ga = GeneticAlgorithm::new(population, 50);

        // Run one generation, selecting top 2 elites
        let (next_pop, max_fitness) = ga.next_generation(2).unwrap();

        // Max fitness should be 0 (from ID 4)
        assert_eq!(max_fitness, 0);

        // Parents selected should be ID 4 (sum 50, diff 0) and ID 1 (sum 30, diff 20)
        let mut has_4 = false;
        let mut has_1 = false;
        for t in next_pop.tuples() {
            let id = t.get_typed::<i64>("id").unwrap();
            if id == 4 {
                has_4 = true;
            }
            if id == 1 {
                has_1 = true;
            }
        }
        assert!(has_4);
        assert!(has_1);

        // Offspring generated from pairing 4 and 1 (both ways: (4,1) and (1,4))
        // Valid pairs: (1,4), (4,1). So 2 offspring + 2 parents = 4 total in next gen.
        assert_eq!(next_pop.cardinality(), 4);
    }
}
