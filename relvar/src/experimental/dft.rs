//! Relational Discrete Fourier Transform (DFT)
//!
//! This module demonstrates how to implement a Discrete Fourier Transform
//! using purely relational algebra. It uses Cartesian products and aggregations
//! to compute the frequency components of a discrete time-domain signal.
//!
//! # Concept
//!
//! The Discrete Fourier Transform computes the frequency domain representation
//! of a discrete signal. We model this as a relational query:
//!
//! - **Signal**: `(n: Int, val: Float)` representing discrete time samples.
//! - **Frequencies**: `(k: Int)` representing the frequency bins.
//!
//! The transform:
//! 1. Cartesian product of Signal and Frequencies.
//! 2. `Extend` to compute the real and imaginary parts of the exponential for each `(n, k)` pair.
//! 3. `Summarize` by grouping on `k` and summing the real and imaginary parts.
//! 4. `Extend` to compute the magnitude for each frequency bin.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Discrete Fourier Transform (DFT).
pub struct RelationalDft;

impl RelationalDft {
    /// Computes the Discrete Fourier Transform of a given signal.
    ///
    /// The input `signal` must have the schema `(n: Int, val: Float)`.
    /// `N` is the total number of samples (and the number of frequency bins).
    ///
    /// Returns a relation with schema `(k: Int, real: Float, imag: Float, magnitude: Float)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::{tuple, Relation, RelationType, TupleType, ScalarType};
    /// use relvar::experimental::dft::RelationalDft;
    ///
    /// let n_heading = TupleType::new()
    ///     .with_attribute("n", ScalarType::Int)
    ///     .with_attribute("val", ScalarType::Float);
    /// let mut signal = Relation::new(RelationType::new(n_heading));
    ///
    /// // DC signal: val = 1.0 for all n
    /// signal.insert(tuple! { n: 0i64, val: 1.0f64 }).unwrap();
    /// signal.insert(tuple! { n: 1i64, val: 1.0f64 }).unwrap();
    /// signal.insert(tuple! { n: 2i64, val: 1.0f64 }).unwrap();
    /// signal.insert(tuple! { n: 3i64, val: 1.0f64 }).unwrap();
    ///
    /// let dft_result = RelationalDft::compute_dft(&signal, 4).unwrap();
    /// assert_eq!(dft_result.cardinality(), 4);
    /// ```
    pub fn compute_dft(signal: &Relation, num_samples: i64) -> Result<Relation, DatabaseError> {
        // 1. Create the frequencies relation (k: Int) from 0 to N-1
        let k_heading = TupleType::new().with_attribute("k", ScalarType::Int);
        let mut frequencies = Relation::new(RelationType::new(k_heading.clone()));
        for k in 0..num_samples {
            let mut vals = std::collections::BTreeMap::new();
            vals.insert("k".to_string(), ScalarValue::Int(k));
            frequencies
                .insert(Tuple::new(k_heading.clone(), vals).unwrap())
                .unwrap();
        }

        // 2. Cartesian product (signal x frequencies)
        let cartesian = signal.join(&frequencies)?;

        // 3. Compute real and imaginary parts for each (n, k)
        let extended = cartesian
            .extend("real_part", ScalarType::Float, move |t| {
                let n = t.get_typed::<i64>("n").unwrap();
                let k = t.get_typed::<i64>("k").unwrap();
                let val = t.get_typed::<f64>("val").unwrap();
                let angle =
                    -2.0 * std::f64::consts::PI * (k as f64) * (n as f64) / (num_samples as f64);
                ScalarValue::Float(val * angle.cos())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("imag_part", ScalarType::Float, move |t| {
                let n = t.get_typed::<i64>("n").unwrap();
                let k = t.get_typed::<i64>("k").unwrap();
                let val = t.get_typed::<f64>("val").unwrap();
                let angle =
                    -2.0 * std::f64::consts::PI * (k as f64) * (n as f64) / (num_samples as f64);
                ScalarValue::Float(val * angle.sin())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Summarize by frequency bin `k`
        let summarized = extended
            .summarize(
                &["k"],
                &[
                    Aggregation::sum_float("real", "real_part"),
                    Aggregation::sum_float("imag", "imag_part"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 5. Compute magnitude
        let final_result = summarized
            .extend("magnitude", ScalarType::Float, |t| {
                let real = t.get_typed::<f64>("real").unwrap();
                let imag = t.get_typed::<f64>("imag").unwrap();
                ScalarValue::Float((real * real + imag * imag).sqrt())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(final_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_dft_dc_signal() -> Result<(), Box<dyn std::error::Error>> {
        let n_heading = TupleType::new()
            .with_attribute("n", ScalarType::Int)
            .with_attribute("val", ScalarType::Float);
        let mut signal = Relation::new(RelationType::new(n_heading));

        // DC signal: val = 1.0 for all n
        let num_samples = 4;
        for n in 0..num_samples {
            signal.insert(tuple! { n: n, val: 1.0f64 })?;
        }

        let dft_result = RelationalDft::compute_dft(&signal, num_samples)?;

        assert_eq!(dft_result.cardinality(), 4);

        for t in dft_result.tuples() {
            let k = t.get_typed::<i64>("k").ok_or("missing k")?;
            let mag = t.get_typed::<f64>("magnitude").ok_or("missing mag")?;
            if k == 0 {
                // DC component should be N
                assert!((mag - 4.0).abs() < 1e-6);
            } else {
                // Other components should be 0
                assert!(mag < 1e-6);
            }
        }

        Ok(())
    }
}
