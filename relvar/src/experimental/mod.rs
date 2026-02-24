//! Experimental features that may be unstable or subject to change.
//!
//! This module contains features that are still in development or being tested.
//! They are available for use but may change significantly in future versions.
//!
//! # Included Features
//!
//! - **Pivot**: Operator to rotate unique values from one column into multiple columns.
//! - **Mock**: Tools for generating random relations for testing.
//! - **Spatial**: Spatial data types.
//! - **Search**: Relational Full-Text Search.
//! - **Image**: Relational Image Processing (RIP).
//! - **Graph**: Relational Graph Analytics (BFS, PageRank).
//! - **AutoML**: Relational Machine Learning (Naive Bayes).
//! - **Time Series**: Relational Time Series Analysis (Moving Averages).
//! - **ECS**: Relational Entity Component System.

pub mod automl;
pub mod ecs;
pub mod graph;
pub mod image;
pub mod mock;
pub mod pivot;
pub mod search;
pub mod spatial;
pub mod timeseries;
