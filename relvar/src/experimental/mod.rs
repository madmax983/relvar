//! Experimental features that push the boundaries of the Relational Model.
//!
//! This module contains features that demonstrate the power and flexibility of
//! pure relational algebra. While "experimental" in the sense that they are
//! not part of the core SQL standard, they are fully functional and show
//! how domains like Graph Analytics, Machine Learning, and Image Processing
//! can be modeled relationally.
//!
//! # The Gallery of Wonders
//!
//! ## 📊 Analytics & ML
//!
//! - **[`graph`]**: Implements graph algorithms (BFS, PageRank) using recursive relational queries.
//!   *Why?* To show that you don't need a graph database to analyze networks.
//! - **[`automl`]**: A Naive Bayes classifier trained purely via `summarize` and `count`.
//!   *Why?* To demonstrate that training a model is just a query.
//! - **[`pivot`]**: Transforms rows into columns (cross-tabulation).
//!   *Why?* Because reporting often requires denormalization.
//!
//! ## 🖼️ Unstructured Data
//!
//! - **[`image`]**: "Relational Image Processing" (RIP). Treats images as relations `(x, y, r, g, b)`
//!   and implements convolution kernels via joins and aggregation.
//!   *Why?* To prove that pixels are just tuples.
//! - **[`search`]**: A full-text search engine using an inverted index relation.
//!   *Why?* To show that a search index is just a relation.
//!
//! ## 🌍 Specialized Types
//!
//! - **[`spatial`]**: Implements Point types and distance calculations using
//!   Relation-Valued Attributes (RVAs) and User-Defined Types (UDTs).
//!   *Why?* To avoid opaque binary blobs for complex types.
//!
//! ## 🛠️ Tools
//!
//! - **[`mock`]**: Generates random relations for testing and benchmarking.

pub mod automl;
pub mod graph;
pub mod image;
pub mod mock;
pub mod pivot;
pub mod search;
pub mod spatial;
