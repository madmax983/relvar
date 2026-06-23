//! Experimental features that may be unstable or subject to change.
//!
//! This module contains features that are still in development or being tested.
//! They are available for use but may change significantly in future versions.
//!
//! # Included Features
//!
//! - **[`automl`](crate::experimental)**: Relational Machine Learning (Naive Bayes).
//! - **[`ecs`](crate::experimental)**: Relational Entity Component System (ECS) pattern.
//! - **[`graph`](crate::experimental)**: Relational Graph Analytics (BFS, PageRank).
//! - **[`image`](crate::experimental)**: Relational Image Processing (RIP).
//! - **[`matrix`](crate::experimental)**: Relational Linear Algebra (Sparse Matrices).
//! - **[`mock`](crate::experimental)**: Tools for generating random relations for testing.
//! - **[`physics`](crate::experimental)**: Relational N-Body Physics Engine.
//! - **[`pivot`](crate::experimental)**: Operator to rotate unique values from one column into multiple columns.
//! - **[`raytracer`](crate::experimental)**: Relational Raytracer.
//! - **[`recommend`](crate::experimental)**: Relational Recommender System (Collaborative Filtering).
//! - **[`search`](crate::experimental)**: Relational Full-Text Search.
//! - **[`spatial`](crate::experimental)**: Spatial data types and operations.
//! - **[`synth`](crate::experimental)**: Relational Audio Synthesizer.
//! - **[`timeseries`](crate::experimental)**: Relational Time Series Analysis (Moving Averages).
//! - **[`turing`](crate::experimental)**: Relational Turing Machine.
//! - **[`vcs`](crate::experimental)**: Relational Version Control System (RelGit).
//! - **[`circuit`](crate::experimental)**: Relational Logic Circuit Simulator.
//! - **[`automata`](crate::experimental)**: Relational Automata (NFA).
//! - **[`gnn`](crate::experimental)**: Relational Graph Neural Network.
//! - **[`enigma`](crate::experimental)**: Relational Enigma Machine.
//! - **[`kmeans`](crate::experimental)**: Relational K-Means Clustering.
//! - **[`vm`](crate::experimental)**: Relational Virtual Machine.
//! - **[`markov`](crate::experimental)**: Relational Markov Chain.
//! - **[`petri_net`](crate::experimental)**: Relational Petri Net Simulator.
//!
//! - **[`build_system`](crate::experimental)**: Relational Build System.

pub(crate) mod automata;
pub(crate) mod automl;
pub(crate) mod blockchain;
pub(crate) mod build_system;
pub(crate) mod cellular_automaton;
pub(crate) mod circuit;
pub(crate) mod ecs;
pub(crate) mod enigma;
pub(crate) mod expert_system;
pub(crate) mod garbage_collector;
pub(crate) mod gnn;
pub(crate) mod graph;
pub(crate) mod image;
pub(crate) mod kmeans;
pub(crate) mod knowledge_graph;
pub(crate) mod matrix;
pub(crate) mod mock;
pub(crate) mod neural_network;

pub(crate) mod dom;
pub(crate) mod genetic_algorithm;
pub(crate) mod graph_neural_network;
pub(crate) mod markov;
pub(crate) mod parser;
/// Relational Petri Net Simulator.
pub(crate) mod petri_net;
pub(crate) mod physics;
pub(crate) mod pivot;
pub(crate) mod raytracer;
pub(crate) mod rbac;
pub(crate) mod recommend;
pub(crate) mod search;
pub(crate) mod spatial;
/// Relational Spreadsheet Simulator.
pub(crate) mod spreadsheet;
pub(crate) mod sudoku;
pub(crate) mod synth;
pub(crate) mod timeseries;
pub(crate) mod turing;
pub(crate) mod vcs;
pub(crate) mod vm;

pub use automata::*;
pub use automl::*;
pub use blockchain::*;
pub use build_system::*;
pub use cellular_automaton::*;
pub use circuit::*;
pub use dom::*;
pub use ecs::*;
pub use enigma::*;
pub use expert_system::*;
pub use garbage_collector::*;
pub use genetic_algorithm::*;
pub use gnn::*;
pub use graph::*;
pub use graph_neural_network::*;
pub use image::*;
pub use kmeans::*;
pub use knowledge_graph::*;
pub use markov::*;
pub use matrix::*;
pub use mock::*;
pub use neural_network::*;
pub use parser::*;
pub use petri_net::*;
pub use physics::*;
pub use pivot::*;
pub use raytracer::*;
pub use rbac::*;
pub use recommend::*;
pub use search::*;
pub use spatial::*;
pub use spreadsheet::*;
pub use sudoku::*;
pub use synth::*;
pub use timeseries::*;
pub use turing::*;
pub use vcs::*;
pub use vm::*;
