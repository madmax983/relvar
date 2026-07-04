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

#[allow(dead_code)]
pub mod automata;
#[allow(dead_code)]
pub mod automl;
#[allow(dead_code)]
pub mod blockchain;
#[allow(dead_code)]
pub mod build_system;
#[allow(dead_code)]
pub mod cellular_automaton;
#[allow(dead_code)]
pub mod circuit;
#[allow(dead_code)]
pub mod ecs;
#[allow(dead_code)]
pub mod enigma;
#[allow(dead_code)]
pub mod expert_system;
#[allow(dead_code)]
pub mod garbage_collector;
#[allow(dead_code)]
pub mod gnn;
#[allow(dead_code)]
pub mod graph;
#[allow(dead_code)]
pub mod image;
#[allow(dead_code)]
pub mod kmeans;
#[allow(dead_code)]
pub mod knowledge_graph;
#[allow(dead_code)]
pub mod matrix;
#[allow(dead_code)]
pub mod mock;
#[allow(dead_code)]
pub mod neural_network;

#[allow(dead_code)]
pub mod dom;
#[allow(dead_code)]
pub mod genetic_algorithm;
#[allow(dead_code)]
pub mod graph_neural_network;
#[allow(dead_code)]
pub mod markov;
#[allow(dead_code)]
pub mod parser;
/// Relational Petri Net Simulator.
#[allow(dead_code)]
pub mod petri_net;
#[allow(dead_code)]
pub mod physics;
#[allow(dead_code)]
pub mod pivot;
#[allow(dead_code)]
pub mod raytracer;
#[allow(dead_code)]
pub mod rbac;
#[allow(dead_code)]
pub mod recommend;
#[allow(dead_code)]
pub mod search;
#[allow(dead_code)]
pub mod spatial;
/// Relational Spreadsheet Simulator.
#[allow(dead_code)]
pub mod spreadsheet;
#[allow(dead_code)]
pub mod sudoku;
#[allow(dead_code)]
pub mod synth;
#[allow(dead_code)]
pub mod timeseries;
#[allow(dead_code)]
pub mod turing;
#[allow(dead_code)]
pub mod vcs;
#[allow(dead_code)]
pub mod vm;
