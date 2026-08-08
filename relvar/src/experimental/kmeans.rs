#![allow(dead_code)]
//! Relational K-Means Clustering
//!
//! This module demonstrates how K-Means clustering can be implemented using purely
//! relational algebra operations.
//!
//! # Concept
//!
//! - **Points**: Relation `(point_id: Int, x: Float, y: Float)`.
//! - **Centroids**: Relation `(cluster_id: Int, c_x: Float, c_y: Float)`.
//!
//! The algorithm:
//! 1. Cartesian product of Points and Centroids.
//! 2. `Extend` to compute distance between each point and centroid.
//! 3. `Summarize` to find the minimum distance for each point, and join to assign the nearest centroid.
//! 4. `Summarize` assigned points by `cluster_id` to compute the mean of `x` and `y` for new centroids.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational K-Means Clustering implementation.
///
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::kmeans::KMeans;
/// // Note: This is a placeholder example
/// ```
pub struct KMeans {
    /// The current centroids. Schema: (cluster_id: Int, c_x: Float, c_y: Float)
    pub centroids: Relation,
}

impl KMeans {
    /// Creates a new KMeans instance with initial centroids.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::kmeans::KMeans;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(initial_centroids: Relation) -> Self {
        Self {
            centroids: initial_centroids,
        }
    }

    /// Assigns points to the nearest centroid.
    ///
    /// Returns a relation with schema: (point_id: Int, cluster_id: Int, x: Float, y: Float)
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::kmeans::KMeans;
    /// // Note: This is a placeholder example
    /// ```
    pub fn assign_clusters(&self, points: &Relation) -> Result<Relation, DatabaseError> {
        // 1. Cartesian product: since points and centroids share no attributes, join is cross join.
        let cartesian = points.join(&self.centroids)?;

        // 2. Compute squared distance
        let distances = cartesian
            .extend("sq_dist", ScalarType::Float, |t| {
                let x = t.get_typed::<f64>("x").unwrap();
                let y = t.get_typed::<f64>("y").unwrap();
                let c_x = t.get_typed::<f64>("c_x").unwrap();
                let c_y = t.get_typed::<f64>("c_y").unwrap();

                let dx = x - c_x;
                let dy = y - c_y;
                ScalarValue::Float(dx * dx + dy * dy)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Find minimum distance per point
        let min_dists = distances
            .summarize(
                &["point_id"],
                &[Aggregation::min(
                    "min_sq_dist",
                    "sq_dist",
                    ScalarType::Float,
                )],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Join back to find candidate clusters
        let min_dists_joinable = min_dists.rename(&[("min_sq_dist", "sq_dist")]);
        let candidates = distances.join(&min_dists_joinable)?;

        // 5. Handle ties by picking the minimum cluster_id
        let exact_assignments = candidates
            .summarize(
                &["point_id"],
                &[Aggregation::min(
                    "cluster_id",
                    "cluster_id",
                    ScalarType::Int,
                )],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 6. Join back with points to include x and y
        points.join(&exact_assignments)
    }

    /// Computes the next iteration of centroids.
    ///
    /// Returns true if centroids changed, false if they converged.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::kmeans::KMeans;
    /// // Note: This is a placeholder example
    /// ```
    pub fn step(&mut self, points: &Relation) -> Result<bool, DatabaseError> {
        let assignments = self.assign_clusters(points)?;

        // Summarize by cluster_id to get counts and sums
        let cluster_stats = assignments
            .summarize(
                &["cluster_id"],
                &[
                    Aggregation::count("count"),
                    Aggregation::sum_float("sum_x", "x"),
                    Aggregation::sum_float("sum_y", "y"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // Compute new c_x and c_y
        let new_centroids = cluster_stats
            .extend("c_x", ScalarType::Float, |t| {
                let sum_x = t.get_typed::<f64>("sum_x").unwrap();
                let count = t.get_typed::<i64>("count").unwrap() as f64;
                ScalarValue::Float(sum_x / count)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("c_y", ScalarType::Float, |t| {
                let sum_y = t.get_typed::<f64>("sum_y").unwrap();
                let count = t.get_typed::<i64>("count").unwrap() as f64;
                ScalarValue::Float(sum_y / count)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["cluster_id", "c_x", "c_y"]);

        // Check for convergence (if difference is empty, they are the same)
        let diff1 = new_centroids
            .difference(&self.centroids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;
        let diff2 = self
            .centroids
            .difference(&new_centroids)
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let changed = !diff1.is_empty() || !diff2.is_empty();

        self.centroids = new_centroids;
        Ok(changed)
    }

    /// Trains the K-Means model for a maximum number of iterations.
    ///
    /// Returns the number of iterations performed.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::kmeans::KMeans;
    /// // Note: This is a placeholder example
    /// ```
    pub fn train(
        &mut self,
        points: &Relation,
        max_iterations: usize,
    ) -> Result<usize, DatabaseError> {
        for i in 0..max_iterations {
            if !self.step(points)? {
                return Ok(i + 1); // Converged
            }
        }
        Ok(max_iterations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;
    use relvar_core::types::{RelationType, TupleType};

    #[test]
    fn test_kmeans_clustering() {
        // Schema for points
        let p_heading = TupleType::new()
            .with_attribute("point_id", ScalarType::Int)
            .with_attribute("x", ScalarType::Float)
            .with_attribute("y", ScalarType::Float);
        let mut points = Relation::new(RelationType::new(p_heading));

        // Group 1: near (1.0, 1.0)
        points
            .insert(tuple! { point_id: 1i64, x: 1.0f64, y: 1.0f64 })
            .unwrap();
        points
            .insert(tuple! { point_id: 2i64, x: 1.1f64, y: 0.9f64 })
            .unwrap();
        points
            .insert(tuple! { point_id: 3i64, x: 0.9f64, y: 1.1f64 })
            .unwrap();

        // Group 2: near (10.0, 10.0)
        points
            .insert(tuple! { point_id: 4i64, x: 10.0f64, y: 10.0f64 })
            .unwrap();
        points
            .insert(tuple! { point_id: 5i64, x: 10.1f64, y: 9.9f64 })
            .unwrap();
        points
            .insert(tuple! { point_id: 6i64, x: 9.9f64, y: 10.1f64 })
            .unwrap();

        // Initial Centroids
        let c_heading = TupleType::new()
            .with_attribute("cluster_id", ScalarType::Int)
            .with_attribute("c_x", ScalarType::Float)
            .with_attribute("c_y", ScalarType::Float);
        let mut centroids = Relation::new(RelationType::new(c_heading));

        // Bad initial centroids
        centroids
            .insert(tuple! { cluster_id: 1i64, c_x: 0.0f64, c_y: 0.0f64 })
            .unwrap();
        centroids
            .insert(tuple! { cluster_id: 2i64, c_x: 2.0f64, c_y: 2.0f64 })
            .unwrap();

        let mut kmeans = KMeans::new(centroids);

        // Train for a max of 10 iterations
        let iters = kmeans.train(&points, 10).unwrap();

        // Should converge quickly
        assert!(iters < 10);

        // Check final assignments
        let assignments = kmeans.assign_clusters(&points).unwrap();

        // Count members in each cluster
        let cluster_counts = assignments
            .summarize(&["cluster_id"], &[Aggregation::count("count")])
            .unwrap();

        assert_eq!(cluster_counts.cardinality(), 2);
        for t in cluster_counts.tuples() {
            let count = t.get_typed::<i64>("count").unwrap();
            assert_eq!(count, 3); // Each cluster should have exactly 3 points
        }
    }
}
