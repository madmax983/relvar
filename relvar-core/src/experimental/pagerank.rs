use crate::algebra::Aggregation;
use crate::error::DatabaseError;
use crate::types::ScalarType;
use crate::values::{Relation, ScalarValue};

/// Evaluates one iteration of the PageRank algorithm using pure relational algebra.
///
/// `edges` must have attributes: `source` (Int), `target` (Int).
/// `ranks` must have attributes: `node` (Int), `rank` (Float).
/// `damping_factor` is the probability of following a link (typically 0.85).
/// `num_nodes` is the total number of nodes in the network.
///
/// # Examples
/// ```
/// use relvar_core::tuple;
/// use relvar_core::types::{RelationType, ScalarType, TupleType};
/// use relvar_core::values::Relation;
/// use relvar_core::experimental::pagerank::pagerank_iteration;
///
/// let edge_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("source", ScalarType::Int)
///         .with_attribute("target", ScalarType::Int)
/// );
/// let mut edges = Relation::new(edge_type);
/// edges.insert(tuple! { source: 1i64, target: 2i64 }).unwrap();
///
/// let rank_type = RelationType::new(
///     TupleType::new()
///         .with_attribute("node", ScalarType::Int)
///         .with_attribute("rank", ScalarType::Float)
/// );
/// let mut ranks = Relation::new(rank_type);
/// ranks.insert(tuple! { node: 1i64, rank: 1.0 }).unwrap();
/// ranks.insert(tuple! { node: 2i64, rank: 0.0 }).unwrap();
///
/// let next_ranks = pagerank_iteration(edges, ranks, 0.85, 2).unwrap();
/// assert_eq!(next_ranks.cardinality(), 1); // Only nodes receiving links get updated
/// ```
pub fn pagerank_iteration(
    edges: Relation,
    ranks: Relation,
    damping_factor: f64,
    num_nodes: usize,
) -> Result<Relation, DatabaseError> {
    let contributions = calculate_contributions(&edges, &ranks)?;
    let new_ranks_summed = distribute_and_aggregate_ranks(&edges, &contributions)?;
    apply_damping_factor(&new_ranks_summed, damping_factor, num_nodes)
}

fn calculate_contributions(edges: &Relation, ranks: &Relation) -> Result<Relation, DatabaseError> {
    // 1. Calculate outgoing edge count for each node
    // Relation with attributes: source (Int), out_degree (Int)
    let out_degrees = edges
        .summarize(&["source"], &[Aggregation::count("out_degree")])
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 2. Join ranks and out_degrees
    // We join ranks (node) with out_degrees (source) by renaming source -> node in out_degrees
    let out_degrees_renamed = out_degrees.rename(&[("source", "node")]);

    // Relation with attributes: node (Int), rank (Float), out_degree (Int)
    let node_info = ranks
        .join(&out_degrees_renamed)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 3. Calculate rank contribution
    // Contribution = rank / out_degree
    // Relation with attributes: node (Int), rank (Float), out_degree (Int), contribution (Float)
    node_info
        .extend("contribution", ScalarType::Float, |t| {
            let rank = t.get_typed::<f64>("rank").unwrap_or(0.0);
            let out_degree = t.get_typed::<i64>("out_degree").unwrap_or(1);
            // Avoid division by zero
            let out_degree_float = if out_degree == 0 {
                1.0
            } else {
                out_degree as f64
            };
            ScalarValue::Float(rank / out_degree_float)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
}

fn distribute_and_aggregate_ranks(
    edges: &Relation,
    contributions: &Relation,
) -> Result<Relation, DatabaseError> {
    // 4. Join edges with contributions to distribute rank
    // edges: source, target
    // contributions: node, rank, out_degree, contribution
    let edges_renamed = edges.rename(&[("source", "node")]);

    // Join on "node"
    // Result attributes: node (source), target, rank, out_degree, contribution
    let distributed = edges_renamed
        .join(contributions)
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // 5. Aggregate received rank per target node
    // Group by target, sum contributions
    // Relation with attributes: target (Int), sum_contribution (Float)
    distributed
        .summarize(
            &["target"],
            &[Aggregation::sum_float("sum_contribution", "contribution")],
        )
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))
}

fn apply_damping_factor(
    new_ranks_summed: &Relation,
    damping_factor: f64,
    num_nodes: usize,
) -> Result<Relation, DatabaseError> {
    // Rename target back to node
    let new_ranks_renamed = new_ranks_summed.rename(&[("target", "node")]);

    // 6. Apply damping factor: new_rank = (1 - d)/N + d * sum_contribution
    let random_jump = (1.0 - damping_factor) / (num_nodes as f64);

    let final_ranks = new_ranks_renamed
        .extend("rank", ScalarType::Float, move |t| {
            let sum_contrib = t.get_typed::<f64>("sum_contribution").unwrap_or(0.0);
            ScalarValue::Float(random_jump + damping_factor * sum_contrib)
        })
        .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

    // Project out the temporary sum_contribution to return only (node, rank)
    Ok(final_ranks.project(&["node", "rank"]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuple;
    use crate::types::{RelationType, TupleType};

    fn setup_relations() -> (Relation, Relation) {
        let edge_heading = TupleType::new()
            .with_attribute("source", ScalarType::Int)
            .with_attribute("target", ScalarType::Int);
        let mut edges = Relation::new(RelationType::new(edge_heading));

        // Node 1 -> Node 2
        // Node 1 -> Node 3
        // Node 2 -> Node 3
        // Node 3 -> Node 1
        edges.insert(tuple! {source: 1i64, target: 2i64}).unwrap();
        edges.insert(tuple! {source: 1i64, target: 3i64}).unwrap();
        edges.insert(tuple! {source: 2i64, target: 3i64}).unwrap();
        edges.insert(tuple! {source: 3i64, target: 1i64}).unwrap();

        let rank_heading = TupleType::new()
            .with_attribute("node", ScalarType::Int)
            .with_attribute("rank", ScalarType::Float);
        let mut ranks = Relation::new(RelationType::new(rank_heading));

        // Initial ranks: 1/3
        ranks.insert(tuple! {node: 1i64, rank: 1.0/3.0}).unwrap();
        ranks.insert(tuple! {node: 2i64, rank: 1.0/3.0}).unwrap();
        ranks.insert(tuple! {node: 3i64, rank: 1.0/3.0}).unwrap();

        (edges, ranks)
    }

    #[test]
    fn test_pagerank_iteration() {
        let (edges, ranks) = setup_relations();
        let num_nodes = 3;
        let damping_factor = 0.85;

        let new_ranks = pagerank_iteration(edges, ranks, damping_factor, num_nodes).unwrap();

        assert_eq!(new_ranks.cardinality(), 3);

        // We can manually verify one iteration
        // out_degree(1) = 2, out_degree(2) = 1, out_degree(3) = 1
        // Node 1 gets rank from Node 3: rank(3) / 1 = 1/3
        // Node 2 gets rank from Node 1: rank(1) / 2 = 1/6
        // Node 3 gets rank from Node 1 + Node 2: rank(1)/2 + rank(2)/1 = 1/6 + 1/3 = 1/2

        // new_rank(1) = (1 - 0.85)/3 + 0.85 * (1/3) = 0.05 + 0.28333 = 0.3333
        // new_rank(2) = 0.05 + 0.85 * (1/6) = 0.05 + 0.14166 = 0.19166
        // new_rank(3) = 0.05 + 0.85 * (1/2) = 0.05 + 0.425 = 0.475

        let rank_sum: f64 = new_ranks
            .tuples()
            .map(|t| t.get_typed::<f64>("rank").unwrap())
            .sum();

        // The sum of ranks should be approximately 1.0, though exact match depends on float math
        assert!((rank_sum - 1.0).abs() < 0.0001);
    }
}
