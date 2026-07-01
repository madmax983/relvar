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

// --- Facade Re-exports ---
pub use automata::RelationalNfa;
pub use automl::NaiveBayesClassifier;
pub use blockchain::Blockchain;
pub use build_system::BuildSystem;
pub use cellular_automaton::CellularAutomaton;
pub use circuit::LogicSimulator;
pub use dom::RelationalDom;
pub use ecs::World;
pub use enigma::EnigmaMachine;
pub use expert_system::ExpertSystem;
pub use garbage_collector::GarbageCollector;
pub use genetic_algorithm::GeneticAlgorithm;
pub use gnn::GraphNeuralNetwork;
pub use graph::Graph;
pub use graph_neural_network::RelationalGNN;
pub use image::KernelTap;
pub use image::apply_kernel;
pub use image::load;
pub use image::save;
pub use kmeans::KMeans;
pub use knowledge_graph::KnowledgeGraph;
pub use knowledge_graph::Term;
pub use knowledge_graph::TriplePattern;
pub use markov::MarkovChain;
pub use matrix::Matrix;
pub use mock::MockRelation;
pub use neural_network::NeuralNetwork;
pub use parser::CykParser;
pub use petri_net::PetriNet;
pub use physics::PhysicsEngine;
pub use pivot::pivot;
pub use raytracer::Scene;
pub use rbac::RelationalRbac;
pub use recommend::CollaborativeFilter;
pub use search::FullTextIndex;
pub use search::tokenize;
pub use spatial::distance;
pub use spatial::point;
pub use spatial::point_type;
pub use spatial::within;
pub use spreadsheet::Spreadsheet;
pub use sudoku::SudokuSolver;
pub use synth::Synth;
pub use timeseries::moving_average;
pub use turing::TuringMachine;
pub use vcs::RelVcs;
pub use vm::RelationalVM;
