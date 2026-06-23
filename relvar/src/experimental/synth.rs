//! Relational Audio Synthesizer
//!
//! This module demonstrates how audio synthesis can be implemented using purely
//! relational algebra operations. It represents a timeline and oscillators as relations,
//! and computes the mixed audio samples using Join, Extend, and Summarize.
//!
//! # Concept
//!
//! - **Timeline**: Relation `(t: Float)` representing discrete time steps.
//! - **Oscillators**: Relation `(osc_id: String, freq: Float, amplitude: Float, wave_type: String)`
//!   representing active sound sources.
//!
//! The synthesis process:
//! 1. Cartesian product of Timeline and Oscillators (since schemas are disjoint, `join` acts as a cross join).
//! 2. `Extend` to compute the sample value for each `(t, osc_id)` pair using the appropriate waveform math.
//! 3. `Summarize` by grouping on `t` and summing the computed sample values across all oscillators to get the mixed audio.

use relvar_core::{
    algebra::Aggregation,
    error::DatabaseError,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Audio Synthesizer.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::Synth;
/// // Note: This is a placeholder example
/// ```
pub struct Synth {
    /// Timeline relation. Schema: `(t: Float)`
    pub timeline: Relation,
    /// Oscillators relation. Schema: `(osc_id: String, freq: Float, amplitude: Float, wave_type: String)`
    pub oscillators: Relation,
}

impl Synth {
    /// Creates a new Synthesizer.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::Synth;
    /// // Note: This is a placeholder example
    /// ```
    pub fn new(timeline: Relation, oscillators: Relation) -> Self {
        Self {
            timeline,
            oscillators,
        }
    }

    /// Helper to generate a timeline relation for a given duration and sample rate.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::Synth;
    /// // Note: This is a placeholder example
    /// ```
    pub fn create_timeline(
        duration_secs: f64,
        sample_rate: u32,
    ) -> Result<Relation, DatabaseError> {
        let heading = TupleType::new().with_attribute("t", ScalarType::Float);
        let mut rel = Relation::new(RelationType::new(heading.clone()));

        let num_samples = (duration_secs * sample_rate as f64).ceil() as u32;
        let dt = 1.0 / sample_rate as f64;

        for i in 0..num_samples {
            let t = (i as f64) * dt;
            let mut vals = std::collections::BTreeMap::new();
            vals.insert("t".to_string(), ScalarValue::Float(t));
            rel.insert(Tuple::new(heading.clone(), vals).unwrap())?;
        }

        Ok(rel)
    }

    /// Computes the synthesized audio samples.
    ///
    /// Computes and returns the synthesized output relation with schema `(t: Float, sample: Float)`.
    /// # Examples
    ///
    /// ```
    /// use relvar::{Relation, RelationType, ScalarType, TupleType};
    /// use relvar::experimental::Synth;
    /// // Note: This is a placeholder example
    /// ```
    pub fn synthesize(&self) -> Result<Relation, DatabaseError> {
        // 1. Cartesian product of timeline and oscillators
        // Since schemas share no attributes, join acts as cross join.
        let cartesian = self.timeline.join(&self.oscillators)?;

        // 2. Compute sample values for each (t, osc_id)
        let samples = cartesian
            .extend("sample_val", ScalarType::Float, |t_tuple| {
                let t = t_tuple.get_typed::<f64>("t").unwrap();
                let freq = t_tuple.get_typed::<f64>("freq").unwrap();
                let amp = t_tuple.get_typed::<f64>("amplitude").unwrap();
                let wave_type = t_tuple.get_typed::<String>("wave_type").unwrap();

                let phase = t * freq * 2.0 * std::f64::consts::PI;

                let val = match wave_type.as_str() {
                    "sine" => phase.sin(),
                    "square" => {
                        if phase.sin() > 0.0 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    "sawtooth" => 2.0 * (t * freq - (0.5 + t * freq).floor()),
                    _ => 0.0, // Default to silence for unknown wave types
                };

                ScalarValue::Float(val * amp)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        // 3. Summarize by grouping on `t` and summing `sample_val`
        let mixed = samples
            .summarize(&["t"], &[Aggregation::sum_float("sample", "sample_val")])
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        Ok(mixed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::tuple;

    #[test]
    fn test_synth_sine_wave() {
        // Create timeline: 4 samples at 4 Hz (t = 0.0, 0.25, 0.5, 0.75)
        let timeline = Synth::create_timeline(1.0, 4).unwrap();
        assert_eq!(timeline.cardinality(), 4);

        // Create oscillator
        let osc_heading = TupleType::new()
            .with_attribute("osc_id", ScalarType::String)
            .with_attribute("freq", ScalarType::Float)
            .with_attribute("amplitude", ScalarType::Float)
            .with_attribute("wave_type", ScalarType::String);
        let mut oscillators = Relation::new(RelationType::new(osc_heading));

        oscillators
            .insert(tuple! {
                osc_id: "osc1",
                freq: 1.0f64,
                amplitude: 0.5f64,
                wave_type: "sine"
            })
            .unwrap();

        let synth = Synth::new(timeline, oscillators);
        let output = synth.synthesize().unwrap();

        // Check output
        assert_eq!(output.cardinality(), 4);

        let mut tuples: Vec<_> = output.tuples().collect();
        tuples.sort_by(|a, b| {
            let ta = a.get_typed::<f64>("t").unwrap();
            let tb = b.get_typed::<f64>("t").unwrap();
            ta.partial_cmp(&tb).unwrap()
        });

        // t = 0.0 => sin(0) * 0.5 = 0.0
        assert!((tuples[0].get_typed::<f64>("sample").unwrap() - 0.0).abs() < 1e-6);

        // t = 0.25 => sin(PI/2) * 0.5 = 0.5
        assert!((tuples[1].get_typed::<f64>("sample").unwrap() - 0.5).abs() < 1e-6);

        // t = 0.5 => sin(PI) * 0.5 = 0.0
        assert!((tuples[2].get_typed::<f64>("sample").unwrap() - 0.0).abs() < 1e-6);

        // t = 0.75 => sin(3*PI/2) * 0.5 = -0.5
        assert!((tuples[3].get_typed::<f64>("sample").unwrap() - -0.5).abs() < 1e-6);
    }

    #[test]
    fn test_synth_mixing() {
        // Timeline: t = 0.0
        let t_heading = TupleType::new().with_attribute("t", ScalarType::Float);
        let mut timeline = Relation::new(RelationType::new(t_heading));
        timeline.insert(tuple! { t: 0.25f64 }).unwrap(); // Quarter second

        // Two oscillators
        let osc_heading = TupleType::new()
            .with_attribute("osc_id", ScalarType::String)
            .with_attribute("freq", ScalarType::Float)
            .with_attribute("amplitude", ScalarType::Float)
            .with_attribute("wave_type", ScalarType::String);
        let mut oscillators = Relation::new(RelationType::new(osc_heading));

        // osc1: 1Hz sine at t=0.25 is sin(PI/2)*0.5 = 0.5
        oscillators
            .insert(tuple! {
                osc_id: "osc1",
                freq: 1.0f64,
                amplitude: 0.5f64,
                wave_type: "sine"
            })
            .unwrap();

        // osc2: 2Hz sine at t=0.25 is sin(PI)*0.2 = 0.0
        oscillators
            .insert(tuple! {
                osc_id: "osc2",
                freq: 2.0f64,
                amplitude: 0.2f64,
                wave_type: "sine"
            })
            .unwrap();

        let synth = Synth::new(timeline, oscillators);
        let output = synth.synthesize().unwrap();

        let t = output.tuples().next().unwrap();
        let sample = t.get_typed::<f64>("sample").unwrap();

        // Total should be 0.5 + 0.0 = 0.5
        assert!((sample - 0.5).abs() < 1e-6);
    }
}
