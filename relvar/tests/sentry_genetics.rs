use rand::prelude::*;
use relvar::experimental::genetics::GeneticOptimizer;
use relvar::{Relation, RelationType, ScalarType, Tuple, TupleType, tuple};

#[test]
fn test_genetic_optimization_maximization() {
    // Problem: Find (x, y) that maximizes x + y, where x, y in [0, 50]
    // Max value is 100.

    let heading = TupleType::new()
        .with_attribute("x", ScalarType::Int)
        .with_attribute("y", ScalarType::Int);
    let rel_type = RelationType::new(heading);

    // Fitness: x + y
    let fitness = |t: &Tuple| -> f64 {
        let x = t.get_typed::<i64>("x").unwrap();
        let y = t.get_typed::<i64>("y").unwrap();
        (x + y) as f64
    };

    // Crossover: Uniform
    let crossover = |p1: &Tuple, p2: &Tuple| -> Tuple {
        let x1 = p1.get_typed::<i64>("x").unwrap();
        let y1 = p1.get_typed::<i64>("y").unwrap();
        let x2 = p2.get_typed::<i64>("x").unwrap();
        let y2 = p2.get_typed::<i64>("y").unwrap();

        let mut rng = thread_rng();

        tuple! {
            x: if rng.r#gen() { x1 } else { x2 },
            y: if rng.r#gen() { y1 } else { y2 }
        }
    };

    // Mutation: Random delta
    let mutation = |t: &Tuple| -> Tuple {
        let x = t.get_typed::<i64>("x").unwrap();
        let y = t.get_typed::<i64>("y").unwrap();
        let mut rng = thread_rng();

        let dx = rng.gen_range(-10..=10);
        let dy = rng.gen_range(-10..=10);

        tuple! {
            x: (x + dx).clamp(0, 50),
            y: (y + dy).clamp(0, 50)
        }
    };

    let optimizer = GeneticOptimizer::new(
        fitness, crossover, mutation, 30,  // population size
        0.4, // mutation rate
        5,   // elite count
    );

    // Initial population (random)
    let mut tuples = Vec::new();
    let mut rng = thread_rng();
    for _ in 0..30 {
        tuples.push(tuple! {
            x: rng.gen_range(0..20), // start low
            y: rng.gen_range(0..20)
        });
    }
    // Note: Cardinality might be < 30 if duplicates are generated
    let mut population = Relation::from_tuples(rel_type, tuples).unwrap();

    println!("Initial Population Size: {}", population.cardinality());

    // Evolve for 30 generations
    for i in 0..30 {
        population = optimizer.evolve(&population).unwrap();

        let best = population
            .tuples()
            .map(|t| t.get_typed::<i64>("x").unwrap() + t.get_typed::<i64>("y").unwrap())
            .max()
            .unwrap_or(0);

        println!(
            "Gen {}: Size={}, Best Fitness={}",
            i,
            population.cardinality(),
            best
        );
    }

    // Check if we improved
    let max_fitness = population
        .tuples()
        .map(|t| {
            let x = t.get_typed::<i64>("x").unwrap();
            let y = t.get_typed::<i64>("y").unwrap();
            x + y
        })
        .max()
        .unwrap();

    // Initial max possible was 19+19=38. We expect to reach closer to 100.
    assert!(
        max_fitness > 50,
        "Optimization should have improved fitness significantly (got {})",
        max_fitness
    );
}
