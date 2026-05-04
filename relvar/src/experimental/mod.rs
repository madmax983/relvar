//! Experimental features that may be unstable or subject to change.
//!
//! This module contains features that are still in development or being tested.
//! They are available for use but may change significantly in future versions.
//!
//! # Included Features
//!
//! - **[`automl`](crate::experimental::automl)**: Relational Machine Learning (Naive Bayes).
//! - **[`ecs`](crate::experimental::ecs)**: Relational Entity Component System (ECS) pattern.
//! - **[`graph`](crate::experimental::graph)**: Relational Graph Analytics (BFS, PageRank).
//! - **[`image`](crate::experimental::image)**: Relational Image Processing (RIP).
//! - **[`matrix`](crate::experimental::matrix)**: Relational Linear Algebra (Sparse Matrices).
//! - **[`mock`](crate::experimental::mock)**: Tools for generating random relations for testing.
//! - **[`physics`](crate::experimental::physics)**: Relational N-Body Physics Engine.
//! - **[`pivot`](crate::experimental::pivot)**: Operator to rotate unique values from one column into multiple columns.
//! - **[`raytracer`](crate::experimental::raytracer)**: Relational Raytracer.
//! - **[`recommend`](crate::experimental::recommend)**: Relational Recommender System (Collaborative Filtering).
//! - **[`search`](crate::experimental::search)**: Relational Full-Text Search.
//! - **[`spatial`](crate::experimental::spatial)**: Spatial data types and operations.
//! - **[`synth`](crate::experimental::synth)**: Relational Audio Synthesizer.
//! - **[`timeseries`](crate::experimental::timeseries)**: Relational Time Series Analysis (Moving Averages).
//! - **[`turing`](crate::experimental::turing)**: Relational Turing Machine.
//! - **[`vcs`](crate::experimental::vcs)**: Relational Version Control System (RelGit).
//! - **[`circuit`](crate::experimental::circuit)**: Relational Logic Circuit Simulator.
//! - **[`automata`](crate::experimental::automata)**: Relational Automata (NFA).
//! - **[`gnn`](crate::experimental::gnn)**: Relational Graph Neural Network.
//! - **[`enigma`](crate::experimental::enigma)**: Relational Enigma Machine.
//! - **[`kmeans`](crate::experimental::kmeans)**: Relational K-Means Clustering.
//! - **[`vm`](crate::experimental::vm)**: Relational Virtual Machine.
//! - **[`markov`](crate::experimental::markov)**: Relational Markov Chain.
//! - **[`petri_net`](crate::experimental::petri_net)**: Relational Petri Net Simulator.
//!
//! - **[`build_system`](crate::experimental::build_system)**: Relational Build System.

pub mod automata;
pub mod automl;
pub mod blockchain;
pub mod build_system;
pub mod cellular_automaton;
pub mod circuit;
pub mod ecs;
pub mod enigma;
pub mod expert_system;
pub mod garbage_collector;
pub mod gnn;
pub mod graph;
pub mod image;
pub mod kmeans;
pub mod knowledge_graph;
pub mod matrix;
pub mod mock;
pub mod neural_network;

pub mod dom;
pub mod game_theory;
pub mod genetic_algorithm;
pub mod graph_neural_network;
pub mod markov;
pub mod parser;
/// Relational Petri Net Simulator.
pub mod petri_net;
pub mod physics;
pub mod pivot;
pub mod raytracer;
pub mod rbac;
pub mod recommend;
pub mod search;
pub mod spatial;
pub mod sudoku;
pub mod synth;
pub mod timeseries;
pub mod turing;
pub mod vcs;
pub mod vm;
