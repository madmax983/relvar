//! Experimental Relational Operations.
//!
//! This module provides experimental implementations of complex algorithms using purely relational algebra.

/// Cellular automata implementation.
pub mod game_of_life;
/// PageRank algorithm implementation.
pub mod pagerank;

/// Knowledge Graph implementation.
pub mod knowledge_graph;

/// Graph Neural Network implementation.
#[cfg(feature = "nova")]
pub mod graph_neural_network;
