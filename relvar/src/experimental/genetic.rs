//! Relational Genetic Algorithm (GA).
//!
//! This module demonstrates how evolutionary computation can be implemented
//! using relational algebra.
//!
//! # Concept
//!
//! A population is a relation. Each tuple is an individual.
//! Attributes are genes.
//!
//! 1. **Evaluation**: `EXTEND` the relation with a `fitness` attribute.
//! 2. **Selection**: `SUMMARIZE` to find average fitness, then `RESTRICT` to keep
//!    only individuals with above-average fitness.
//! 3. **Crossover**: Join survivors with themselves (or shuffle) to create pairs,
//!    then mix attributes to form offspring.
//! 4. **Mutation**: Randomly perturb attributes.
//!
//! # Example
//!
//! ```
//! use relvar::experimental::genetic::{GeneticAlgorithm, EvolutionConfig};
//! use relvar_core::types::{RelationType, TupleType, ScalarType};
//! use relvar_core::values::{Relation, ScalarValue};
//! use relvar_core::tuple;
//!
//! // Problem: Find x, y such that x + y is maximized (max 100 each)
//!
//! // 1. Define Gene Schema
//! let gene_type = TupleType::new()
//!     .with_attribute("x", ScalarType::Int)
//!     .with_attribute("y", ScalarType::Int);
//!
//! // 2. Create Initial Population
//! let mut pop = Relation::new(RelationType::new(gene_type));
//! pop.insert(tuple! { x: 10, y: 10 }).unwrap();
//! pop.insert(tuple! { x: 20, y: 20 }).unwrap();
//! // ... (more individuals)
//!
//! // 3. Configure GA
//! let algo = GeneticAlgorithm::new(EvolutionConfig {
//!     generations: 10,
//!     mutation_rate: 0.1,
//!     ..Default::default()
//! });
//!
//! // 4. Evolve
//! let best_pop = algo.evolve(pop, |t| {
//!     let x = t.get_typed::<i64>("x").unwrap();
//!     let y = t.get_typed::<i64>("y").unwrap();
//!     // Simple fitness: x + y
//!     // We return float for fitness
//!     (x + y) as f64
//! }).unwrap();
//! ```

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use relvar_core::algebra::summarize::Aggregation;
use relvar_core::error::DatabaseError;
use relvar_core::types::ScalarType;
use relvar_core::values::{Relation, ScalarValue, Tuple};
use std::collections::BTreeMap;

/// Configuration for the Genetic Algorithm.
#[derive(Debug, Clone)]
pub struct EvolutionConfig {
    /// Number of generations to evolve.
    pub generations: usize,
    /// Probability (0.0 - 1.0) of a gene mutating.
    pub mutation_rate: f64,
    /// Random seed for reproducibility.
    pub seed: Option<u64>,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            generations: 10,
            mutation_rate: 0.1,
            seed: None,
        }
    }
}

/// The Genetic Algorithm runner.
pub struct GeneticAlgorithm {
    config: EvolutionConfig,
}

impl GeneticAlgorithm {
    /// Creates a new Genetic Algorithm runner.
    pub fn new(config: EvolutionConfig) -> Self {
        Self { config }
    }

    /// Evolves the population for a specified number of generations.
    ///
    /// # Arguments
    ///
    /// * `initial_population` - The starting relation (individuals).
    /// * `fitness_fn` - A function that evaluates a tuple and returns a fitness score (f64).
    pub fn evolve<F>(
        &self,
        initial_population: Relation,
        fitness_fn: F,
    ) -> Result<Relation, DatabaseError>
    where
        F: Fn(&Tuple) -> f64 + Clone,
    {
        let mut population = initial_population;
        let mut rng = if let Some(seed) = self.config.seed {
            StdRng::seed_from_u64(seed)
        } else {
            StdRng::from_entropy()
        };

        let target_count = population.cardinality(); // Maintain population size

        for _generation in 0..self.config.generations {
            if population.is_empty() {
                break;
            }

            // 1. Evaluate Fitness
            // Extend with "fitness" attribute
            let f = fitness_fn.clone();
            let population_with_fitness = population
                .extend("fitness", ScalarType::Float, move |t| {
                    ScalarValue::Float(f(t))
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // 2. Selection (Survival of the Fittest)
            // Calculate average fitness
            let stats = population_with_fitness
                .summarize(&[], &[Aggregation::avg("avg_fitness", "fitness")])
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            let avg_fitness = if let Some(tuple) = stats.tuples().next() {
                tuple.get_typed::<f64>("avg_fitness").unwrap_or(0.0)
            } else {
                0.0
            };

            // Restrict: Keep individuals with fitness >= average
            let survivors_with_fitness = population_with_fitness
                .restrict(|t| t.get_typed::<f64>("fitness").unwrap_or(0.0) >= avg_fitness);

            // Remove fitness column to get back to original schema
            let original_attrs: Vec<&str> = population
                .relation_type()
                .heading()
                .attribute_names()
                .map(|s| s.as_str())
                .collect();

            let survivors = survivors_with_fitness.project(&original_attrs);

            if survivors.is_empty() {
                population = survivors;
                break;
            }

            // 3. Crossover & Mutation (Reproduction)
            let survivors_vec: Vec<Tuple> = survivors.tuples().cloned().collect();

            // Elitism: Keep survivors in the new population
            let mut offspring_tuples = survivors_vec.clone();

            // Fill the rest with offspring
            while offspring_tuples.len() < target_count {
                // Pick two parents randomly
                let p1 = &survivors_vec[rng.gen_range(0..survivors_vec.len())];
                let p2 = &survivors_vec[rng.gen_range(0..survivors_vec.len())];

                let child = self.crossover_and_mutate(p1, p2, &mut rng)?;
                offspring_tuples.push(child);
            }

            // Reconstruct population Relation
            // Duplicates will be removed by set semantics, so we might end up with fewer tuples than target_count.
            // That's acceptable for this relational implementation.
            population =
                Relation::from_tuples(population.relation_type().clone(), offspring_tuples)?;
        }

        Ok(population)
    }

    fn crossover_and_mutate(
        &self,
        p1: &Tuple,
        p2: &Tuple,
        rng: &mut StdRng,
    ) -> Result<Tuple, DatabaseError> {
        let mut child_values = BTreeMap::new();
        let heading = p1.tuple_type(); // Assuming p1 and p2 have same heading

        for (attr, type_) in heading.attributes() {
            // Crossover: Pick gene from P1 or P2 (Uniform Crossover)
            let val = if rng.gen_bool(0.5) {
                p1.get(attr).unwrap()
            } else {
                p2.get(attr).unwrap()
            };

            // Mutation
            let final_val = if rng.gen_bool(self.config.mutation_rate) {
                self.mutate_value(val, type_, rng)
            } else {
                val.clone()
            };

            child_values.insert(attr.clone(), final_val);
        }

        Tuple::new(heading.clone(), child_values)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
    }

    fn mutate_value(&self, val: &ScalarValue, type_: &ScalarType, rng: &mut StdRng) -> ScalarValue {
        match (val, type_) {
            (ScalarValue::Int(v), ScalarType::Int) => {
                // Mutate integer: +/- small amount or random
                // Let's do small drift: +/- 1 to 10
                let drift = rng.gen_range(-10..=10);
                ScalarValue::Int(v + drift)
            }
            (ScalarValue::Float(v), ScalarType::Float) => {
                // Mutate float: +/- 10%
                let factor = rng.gen_range(0.9..=1.1);
                ScalarValue::Float(v * factor)
            }
            (ScalarValue::Bool(v), ScalarType::Bool) => ScalarValue::Bool(!v),
            // For other types, maybe replace with random?
            // For now, keep as is if we don't know how to mutate nicely
            _ => val.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_maximize_simple_integer() {
        // Gene: "x" (Int)
        // Fitness: x (Goal: Maximize x)
        let heading = TupleType::new().with_attribute("x", ScalarType::Int);
        let rel_type = RelationType::new(heading.clone());
        let mut initial_pop = Relation::new(rel_type.clone());

        // Start with low values
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..50 {
            let x = rng.gen_range(0..20);
            let mut values = BTreeMap::new();
            values.insert("x".to_string(), ScalarValue::Int(x));
            let _ = initial_pop.insert(Tuple::new(heading.clone(), values).unwrap());
        }

        let config = EvolutionConfig {
            generations: 50,
            mutation_rate: 0.1, // Reduced mutation rate
            seed: Some(42),
        };
        let ga = GeneticAlgorithm::new(config);

        // Fitness function: simple identity
        let result = ga
            .evolve(initial_pop, |t| t.get_typed::<i64>("x").unwrap() as f64)
            .unwrap();

        // Check that we have improved
        // Max x in initial was < 20.
        // After 20 generations of "add drift", we expect some to be much higher.

        let mut max_x = i64::MIN;
        for t in result.tuples() {
            let x = t.get_typed::<i64>("x").unwrap();
            if x > max_x {
                max_x = x;
            }
        }

        println!("Max x after evolution: {}", max_x);
        assert!(max_x > 40, "Evolution failed to maximize x, got {}", max_x);
    }
}
