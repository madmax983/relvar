//! Relational K-Means Clustering.
//!
//! This module implements K-Means Clustering using purely relational algebra
//! operations. It demonstrates how iterative machine learning algorithms
//! can be computed directly within a relational engine without extracting data.
//!
//! # Concept
//!
//! We represent data points as a relation: `(point_id, x, y)`.
//! Centroids are represented as another relation: `(cluster_id, cx, cy)`.
//!
//! Using relational operators, we can iteratively:
//! 1. Calculate distances from points to centroids using `cross_join` and `extend`.
//! 2. Find the closest centroid for each point using `summarize` with `min` distance,
//!    then `join` to find which cluster that minimum distance corresponds to.
//! 3. Update centroids by computing the `avg` of `x` and `y` for points in each cluster.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue},
};

/// Relational K-Means Clustering.
pub struct KMeans {
    /// The relation containing the data points. Schema: (point_id_attr, x_attr, y_attr)
    points: Relation,
    point_id_attr: String,
    x_attr: String,
    y_attr: String,
}

impl KMeans {
    /// Creates a new KMeans instance.
    ///
    /// # Arguments
    ///
    /// * `points` - The relation containing data points.
    /// * `point_id_attr` - The attribute name for the point identifier.
    /// * `x_attr` - The attribute name for the X coordinate (must be Float).
    /// * `y_attr` - The attribute name for the Y coordinate (must be Float).
    pub fn new(points: Relation, point_id_attr: &str, x_attr: &str, y_attr: &str) -> Self {
        Self {
            points,
            point_id_attr: point_id_attr.to_string(),
            x_attr: x_attr.to_string(),
            y_attr: y_attr.to_string(),
        }
    }

    /// Runs the K-Means algorithm for a specified number of iterations.
    ///
    /// # Arguments
    ///
    /// * `iterations` - Number of iterations to run.
    /// * `initial_centroids` - The initial centroids relation. Schema: (cluster_id_attr, cx_attr, cy_attr).
    ///   - `cluster_id_attr` must be of type Int.
    ///   - `cx_attr` and `cy_attr` must be Float.
    /// * `cluster_id_attr` - The attribute name for the cluster identifier in `initial_centroids`.
    /// * `cx_attr` - The attribute name for the centroid X coordinate.
    /// * `cy_attr` - The attribute name for the centroid Y coordinate.
    ///
    /// Returns the final cluster assignments: `(point_id, cluster_id)`.
    pub fn run(
        &self,
        iterations: usize,
        initial_centroids: Relation,
        cluster_id_attr: &str,
        cx_attr: &str,
        cy_attr: &str,
    ) -> Result<Relation, DatabaseError> {
        let mut centroids = initial_centroids;

        // Ensure we have something to work with
        if self.points.is_empty() || centroids.is_empty() {
            let heading = TupleType::new()
                .with_attribute(
                    self.point_id_attr.clone(),
                    self.points
                        .relation_type()
                        .heading()
                        .get_attribute_type(&self.point_id_attr)
                        .unwrap()
                        .clone(),
                )
                .with_attribute(
                    cluster_id_attr.to_string(),
                    centroids
                        .relation_type()
                        .heading()
                        .get_attribute_type(cluster_id_attr)
                        .unwrap()
                        .clone(),
                );
            return Ok(Relation::new(RelationType::new(heading)));
        }

        let mut assignments = Relation::new(RelationType::new(TupleType::new()));

        for _ in 0..iterations {
            // 1. Calculate distances from all points to all centroids
            // Since points and centroids share no common attributes (they shouldn't),
            // a natural join acts as a cartesian product.
            let product = self.points.join(&centroids)?;

            // Calculate squared distance to avoid sqrt cost, sufficient for assignment.
            let distances = product
                .extend("dist_sq", ScalarType::Float, |t| {
                    let px = t.get_typed::<f64>(&self.x_attr).unwrap();
                    let py = t.get_typed::<f64>(&self.y_attr).unwrap();
                    let cx = t.get_typed::<f64>(cx_attr).unwrap();
                    let cy = t.get_typed::<f64>(cy_attr).unwrap();

                    let dx = px - cx;
                    let dy = py - cy;
                    ScalarValue::Float(dx * dx + dy * dy)
                })
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // 2. Find the minimum distance for each point
            let min_distances = distances
                .summarize(
                    &[&self.point_id_attr],
                    &[Aggregation::min("min_dist", "dist_sq", ScalarType::Float)],
                )
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Rename min_dist back to dist_sq so we can join it back to distances
            let renamed_min = min_distances.rename(&[("min_dist", "dist_sq")]);

            // Join back to distances to find which cluster_id had that minimum distance
            // Result schema: (point_id, x, y, cluster_id, cx, cy, dist_sq)
            let closest = distances.join(&renamed_min)?;

            // Since multiple centroids could theoretically be equidistant, we group by point_id
            // and pick the min cluster_id to break ties deterministically.
            let tie_breaker = closest
                .summarize(
                    &[&self.point_id_attr],
                    &[Aggregation::min(
                        "best_cluster",
                        cluster_id_attr,
                        ScalarType::Int,
                    )],
                )
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            let renamed_tie_breaker = tie_breaker.rename(&[("best_cluster", cluster_id_attr)]);

            // Update assignments for the result: (point_id, cluster_id)
            assignments = renamed_tie_breaker.clone();

            // 3. Recompute centroids based on new assignments
            // Join assignments with points to get (point_id, cluster_id, x, y)
            let assigned_points = assignments.join(&self.points)?;

            // Compute the average x and y for each cluster
            let new_centroids = assigned_points
                .summarize(
                    &[cluster_id_attr],
                    &[
                        Aggregation::avg("new_cx", &self.x_attr),
                        Aggregation::avg("new_cy", &self.y_attr),
                    ],
                )
                .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

            // Rename new_cx and new_cy to match centroid schema
            centroids = new_centroids.rename(&[("new_cx", cx_attr), ("new_cy", cy_attr)]);
        }

        Ok(assignments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_kmeans_clustering() {
        // Points Schema: (id: Int, px: Float, py: Float)
        let points_heading = TupleType::new()
            .with_attribute("id", ScalarType::Int)
            .with_attribute("px", ScalarType::Float)
            .with_attribute("py", ScalarType::Float);

        let mut points = Relation::new(RelationType::new(points_heading));

        // Cluster 1 (around origin)
        points
            .insert(tuple! { id: 1i64, px: 0.1, py: 0.1 })
            .unwrap();
        points
            .insert(tuple! { id: 2i64, px: -0.1, py: -0.1 })
            .unwrap();
        points
            .insert(tuple! { id: 3i64, px: 0.1, py: -0.1 })
            .unwrap();
        points
            .insert(tuple! { id: 4i64, px: -0.1, py: 0.1 })
            .unwrap();

        // Cluster 2 (around 10, 10)
        points
            .insert(tuple! { id: 5i64, px: 10.1, py: 10.1 })
            .unwrap();
        points
            .insert(tuple! { id: 6i64, px: 9.9, py: 9.9 })
            .unwrap();
        points
            .insert(tuple! { id: 7i64, px: 10.1, py: 9.9 })
            .unwrap();
        points
            .insert(tuple! { id: 8i64, px: 9.9, py: 10.1 })
            .unwrap();

        // Initial Centroids Schema: (c_id: Int, cx: Float, cy: Float)
        let centroids_heading = TupleType::new()
            .with_attribute("c_id", ScalarType::Int)
            .with_attribute("cx", ScalarType::Float)
            .with_attribute("cy", ScalarType::Float);

        let mut centroids = Relation::new(RelationType::new(centroids_heading));
        // Intentionally slightly off initially
        centroids
            .insert(tuple! { c_id: 100i64, cx: 2.0, cy: 2.0 })
            .unwrap();
        centroids
            .insert(tuple! { c_id: 200i64, cx: 8.0, cy: 8.0 })
            .unwrap();

        let kmeans = KMeans::new(points, "id", "px", "py");

        // Run for 3 iterations
        let assignments = kmeans.run(3, centroids, "c_id", "cx", "cy").unwrap();

        assert_eq!(assignments.cardinality(), 8);

        // Check if assignments are correct
        let mut cluster1_count = 0;
        let mut cluster2_count = 0;
        let mut cluster1_id = -1;
        let mut cluster2_id = -1;

        for t in assignments.tuples() {
            let id = t.get_typed::<i64>("id").unwrap();
            let c_id = t.get_typed::<i64>("c_id").unwrap();

            if id <= 4 {
                if cluster1_id == -1 {
                    cluster1_id = c_id;
                }
                assert_eq!(
                    c_id, cluster1_id,
                    "Point {} should be in cluster {}",
                    id, cluster1_id
                );
                cluster1_count += 1;
            } else {
                if cluster2_id == -1 {
                    cluster2_id = c_id;
                }
                assert_eq!(
                    c_id, cluster2_id,
                    "Point {} should be in cluster {}",
                    id, cluster2_id
                );
                cluster2_count += 1;
            }
        }

        assert_eq!(cluster1_count, 4);
        assert_eq!(cluster2_count, 4);
        assert_ne!(cluster1_id, cluster2_id);
    }
}
