//! Relational Fourier Transform
//!
//! This module demonstrates how the Discrete Fourier Transform (DFT) can be implemented
//! using purely relational algebra operations. It represents a signal as a relation,
//! generating the transformation matrix via Cartesian products, and computes the
//! result using Join, Extend, and Summarize.
//!
//! # Concept
//!
//! - **Signal**: Relation `(t: Int, val: Float)` representing discrete time samples.
//! - **Basis**: We compute the transform dynamically using mathematical operations on
//!   frequencies (k) and time indices (t).
//!
//! The transformation process for DFT:
//! 1. Generate an index of frequencies `k` from 0 to N-1.
//! 2. Cartesian product of Signal `(t, val)` and Frequencies `(k)`.
//! 3. `Extend` to compute the real and imaginary components for each `(k, t)` pair:
//!    - real = val * cos(2 * PI * k * t / N)
//!    - imag = val * -sin(2 * PI * k * t / N)
//! 4. `Summarize` by grouping on `k` and summing the real and imaginary components to
//!    get the transformed signal in the frequency domain.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Fourier Transform.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::fourier::RelationalFourier;
/// // Note: This is a placeholder example
/// ```
pub struct RelationalFourier;

impl RelationalFourier {
    /// Computes the Discrete Fourier Transform (DFT) of a given signal.
    ///
    /// The input signal should be a relation with schema `(t: Int, val: Float)`.
    /// The result will be a relation with schema `(k: Int, real: Float, imag: Float)` representing
    /// the frequency domain. `N` is the number of samples.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::fourier::RelationalFourier;
    /// // Note: This is a placeholder example
    /// ```
    pub fn dft(signal: &Relation, num_samples: usize) -> Result<Relation, DatabaseError> {
        let n_f64 = num_samples as f64;

        // 1. Generate frequencies `k`
        let k_heading = TupleType::new().with_attribute("k", ScalarType::Int);
        let mut k_rel = Relation::new(RelationType::new(k_heading.clone()));

        for k in 0..num_samples {
            let mut vals = std::collections::BTreeMap::new();
            vals.insert("k".to_string(), ScalarValue::Int(k as i64));
            k_rel.insert(Tuple::new(k_heading.clone(), vals).unwrap())?;
        }

        // 2. Cartesian product of Signal and Frequencies
        let cartesian = signal.join(&k_rel)?;

        // 3. Extend to compute real and imaginary parts
        let components = cartesian
            .extend("real_part", ScalarType::Float, move |tuple| {
                let t = tuple.get_typed::<i64>("t").unwrap() as f64;
                let val = tuple.get_typed::<f64>("val").unwrap();
                let k = tuple.get_typed::<i64>("k").unwrap() as f64;

                let angle = 2.0 * std::f64::consts::PI * k * t / n_f64;
                ScalarValue::Float(val * angle.cos())
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .extend("imag_part", ScalarType::Float, move |tuple| {
                let t = tuple.get_typed::<i64>("t").unwrap() as f64;
                let val = tuple.get_typed::<f64>("val").unwrap();
                let k = tuple.get_typed::<i64>("k").unwrap() as f64;

                let angle = 2.0 * std::f64::consts::PI * k * t / n_f64;
                ScalarValue::Float(val * -angle.sin()) // -sin for forward DFT
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 4. Summarize by `k` to sum the components
        let result = components
            .summarize(
                &["k"],
                &[
                    Aggregation::sum_float("real", "real_part"),
                    Aggregation::sum_float("imag", "imag_part"),
                ],
            )
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    fn create_signal(samples: &[f64]) -> Relation {
        let heading = TupleType::new()
            .with_attribute("t", ScalarType::Int)
            .with_attribute("val", ScalarType::Float);

        let mut rel = Relation::new(RelationType::new(heading.clone()));

        for (i, &val) in samples.iter().enumerate() {
            rel.insert(tuple! { t: i as i64, val: val }).unwrap();
        }

        rel
    }

    #[test]
    fn test_dft_dc_signal() {
        // DC signal: [1.0, 1.0, 1.0, 1.0]
        let signal = create_signal(&[1.0, 1.0, 1.0, 1.0]);
        let dft = RelationalFourier::dft(&signal, 4).unwrap();

        assert_eq!(dft.cardinality(), 4);

        let mut results = std::collections::HashMap::new();
        for tuple in dft.tuples() {
            let k = tuple.get_typed::<i64>("k").unwrap();
            let real = tuple.get_typed::<f64>("real").unwrap();
            let imag = tuple.get_typed::<f64>("imag").unwrap();
            results.insert(k, (real, imag));
        }

        // k=0 should have amplitude 4.0 (DC component)
        assert!((results[&0].0 - 4.0).abs() < 1e-6);
        assert!((results[&0].1 - 0.0).abs() < 1e-6);

        // Others should be 0
        for k in 1..4 {
            assert!((results[&k].0 - 0.0).abs() < 1e-6);
            assert!((results[&k].1 - 0.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_dft_nyquist() {
        // Nyquist frequency signal: [1.0, -1.0, 1.0, -1.0]
        let signal = create_signal(&[1.0, -1.0, 1.0, -1.0]);
        let dft = RelationalFourier::dft(&signal, 4).unwrap();

        assert_eq!(dft.cardinality(), 4);

        let mut results = std::collections::HashMap::new();
        for tuple in dft.tuples() {
            let k = tuple.get_typed::<i64>("k").unwrap();
            let real = tuple.get_typed::<f64>("real").unwrap();
            let imag = tuple.get_typed::<f64>("imag").unwrap();
            results.insert(k, (real, imag));
        }

        // Nyquist frequency is at k = N/2 = 2
        assert!((results[&2].0 - 4.0).abs() < 1e-6);
        assert!((results[&2].1 - 0.0).abs() < 1e-6);

        // Others should be 0
        assert!((results[&0].0 - 0.0).abs() < 1e-6);
        assert!((results[&1].0 - 0.0).abs() < 1e-6);
        assert!((results[&3].0 - 0.0).abs() < 1e-6);
    }
}
