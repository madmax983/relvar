//! Experimental Relational Operations.
//!
//! This module provides experimental implementations of complex algorithms using purely relational algebra.

/// Cellular automata implementation.
pub(crate) mod game_of_life;
/// PageRank algorithm implementation.
pub(crate) mod pagerank;

/// Knowledge Graph implementation.
pub(crate) mod knowledge_graph;

pub use game_of_life::*;
pub use knowledge_graph::*;
pub use pagerank::*;
