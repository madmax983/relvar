//! Relational Genetic Algorithm (RGA)
//!
//! This module implements a genetic algorithm optimizer that operates on relations.
//!
//! # Concept
//!
//! - **Population**: A `Relation` where each tuple represents an individual.
//! - **Genome**: The attributes of the tuple.
//! - **Evolution**: Applying selection, crossover, and mutation to generate a new relation.
//!
//! # Relational vs. Genetic
//!
//! Genetic algorithms typically allow duplicate individuals. However, the Relational Model
//! strictly enforces set semantics (no duplicates). This implementation adheres to the
//! Relational Model: if evolution produces identical individuals, they are naturally
//! deduplicated. This means the population size may shrink as the algorithm converges
//! to an optimal solution.

use rand::prelude::*;
use relvar_core::values::{Relation, Tuple};
use std::cmp::Ordering;

/// Type alias for the fitness function.
pub type FitnessFunc = Box<dyn Fn(&Tuple) -> f64 + Send + Sync>;
/// Type alias for the crossover function.
pub type CrossoverFunc = Box<dyn Fn(&Tuple, &Tuple) -> Tuple + Send + Sync>;
/// Type alias for the mutation function.
pub type MutationFunc = Box<dyn Fn(&Tuple) -> Tuple + Send + Sync>;

/// A genetic algorithm optimizer that evolves a population (Relation).
pub struct GeneticOptimizer {
    fitness_func: FitnessFunc,
    crossover_func: CrossoverFunc,
    mutation_func: MutationFunc,
    population_size: usize,
    mutation_rate: f64,
    elite_count: usize,
}

impl GeneticOptimizer {
    /// Creates a new Genetic Optimizer.
    ///
    /// # Arguments
    ///
    /// * `fitness_func` - Function to evaluate the fitness of an individual (Tuple).
    /// * `crossover_func` - Function to combine two parents into an offspring.
    /// * `mutation_func` - Function to randomly mutate an individual.
    /// * `population_size` - Target size of the population.
    /// * `mutation_rate` - Probability of mutation for each offspring.
    /// * `elite_count` - Number of top individuals to carry over unchanged.
    pub fn new(
        fitness_func: impl Fn(&Tuple) -> f64 + Send + Sync + 'static,
        crossover_func: impl Fn(&Tuple, &Tuple) -> Tuple + Send + Sync + 'static,
        mutation_func: impl Fn(&Tuple) -> Tuple + Send + Sync + 'static,
        population_size: usize,
        mutation_rate: f64,
        elite_count: usize,
    ) -> Self {
        Self {
            fitness_func: Box::new(fitness_func),
            crossover_func: Box::new(crossover_func),
            mutation_func: Box::new(mutation_func),
            population_size,
            mutation_rate,
            elite_count,
        }
    }

    /// Evolves the population for one generation.
    ///
    /// # Returns
    ///
    /// A new `Relation` representing the next generation. Note that due to set semantics,
    /// the cardinality of the returned relation may be less than `population_size` if
    /// the population converges to identical individuals.
    pub fn evolve(&self, population: &Relation) -> Result<Relation, String> {
        if population.is_empty() {
            return Ok(population.clone());
        }

        // 1. Evaluate fitness
        let mut scored_population: Vec<(f64, Tuple)> = Vec::with_capacity(population.cardinality());
        for tuple in population.tuples() {
            let score = (self.fitness_func)(tuple);
            scored_population.push((score, tuple.clone()));
        }

        // 2. Sort by fitness descending
        scored_population.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

        // 3. Select Elites
        let mut new_population_tuples = Vec::with_capacity(self.population_size);
        let actual_elites = self.elite_count.min(scored_population.len());

        for (_, tuple) in scored_population.iter().take(actual_elites) {
            new_population_tuples.push(tuple.clone());
        }

        // 4. Generate Offspring
        let mut rng = thread_rng();
        // We attempt to fill up to population_size. However, since Relation is a Set,
        // duplicates will be removed. We generate exactly population_size - elites candidates,
        // accepting that duplicates might be dropped.
        let num_offspring = self.population_size.saturating_sub(actual_elites);

        for _ in 0..num_offspring {
            // Tournament selection
            let p1 = self.tournament_select(&scored_population, &mut rng);
            let p2 = self.tournament_select(&scored_population, &mut rng);

            let mut offspring = (self.crossover_func)(p1, p2);

            if rng.r#gen::<f64>() < self.mutation_rate {
                offspring = (self.mutation_func)(&offspring);
            }

            // Ensure offspring conforms to type
            if offspring.tuple_type() != population.relation_type().heading() {
                return Err("Offspring tuple type mismatch".to_string());
            }

            new_population_tuples.push(offspring);
        }

        // Create new relation (automatically deduplicates)
        Relation::from_tuples(population.relation_type().clone(), new_population_tuples)
            .map_err(|e| e.to_string())
    }

    fn tournament_select<'a, R: Rng>(
        &self,
        scored_population: &'a [(f64, Tuple)],
        rng: &mut R,
    ) -> &'a Tuple {
        let k = 3;
        let idx = rng.gen_range(0..scored_population.len());
        let mut best = &scored_population[idx];

        for _ in 1..k {
            let idx = rng.gen_range(0..scored_population.len());
            let candidate = &scored_population[idx];
            if candidate.0 > best.0 {
                best = candidate;
            }
        }
        &best.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, ScalarType, TupleType};

    #[test]
    fn test_simple_evolution() {
        // Goal: Maximize x, where x is in [0, 100]
        let heading = TupleType::new().with_attribute("x", ScalarType::Int);
        let rel_type = RelationType::new(heading.clone());

        // Fitness: x
        let fitness = |t: &Tuple| -> f64 { t.get_typed::<i64>("x").unwrap() as f64 };

        // Crossover: Average of x
        let crossover = |p1: &Tuple, p2: &Tuple| -> Tuple {
            let x1 = p1.get_typed::<i64>("x").unwrap();
            let x2 = p2.get_typed::<i64>("x").unwrap();
            let new_x = (x1 + x2) / 2;
            tuple! { x: new_x }
        };

        // Mutation: x + random(-5, 5)
        let mutation = |t: &Tuple| -> Tuple {
            let x = t.get_typed::<i64>("x").unwrap();
            let mut rng = thread_rng();
            let delta = rng.gen_range(-5..=5);
            let new_x = (x + delta).clamp(0, 100);
            tuple! { x: new_x }
        };

        let optimizer = GeneticOptimizer::new(fitness, crossover, mutation, 10, 0.5, 2);

        // Initial population: low values
        let mut tuples = Vec::new();
        for i in 0..5 {
            tuples.push(tuple! { x: i });
        }
        let population = Relation::from_tuples(rel_type, tuples).unwrap();

        let next_gen = optimizer.evolve(&population).unwrap();

        // Check that we have valid tuples
        assert!(!next_gen.is_empty());
        for t in next_gen.tuples() {
            let x = t.get_typed::<i64>("x").unwrap();
            assert!((0..=100).contains(&x));
        }
    }
}
