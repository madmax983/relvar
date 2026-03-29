//! Relational Audio Synthesizer.
//!
//! This module demonstrates how a simple audio synthesizer can be implemented using
//! pure relational algebra operations. It treats an audio waveform as a relation of
//! time and amplitude `(t, sample)`.
//!
//! # Concept
//!
//! We represent multiple oscillators as a relation: `(osc_id, freq, amp, waveform)`.
//!
//! To synthesize audio:
//! 1. Generate a relation of discrete time steps: `(t)`.
//! 2. Cross join time steps with the oscillators.
//! 3. Extend to compute the sample value for each oscillator at each time step.
//! 4. Summarize (group by time) and sum the sample values to mix the audio.

use relvar_core::{
    algebra::Aggregation,
    tuple,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue, Tuple},
};

/// A Relational Audio Synthesizer.
pub struct Synth {
    /// The sampling rate in Hz (e.g., 44100).
    pub sample_rate: u32,
}

impl Synth {
    /// Creates a new Synth.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - The audio sampling rate in Hz.
    pub fn new(sample_rate: u32) -> Self {
        Self { sample_rate }
    }

    /// Synthesizes audio by evaluating the provided oscillators over the given duration.
    ///
    /// # Arguments
    ///
    /// * `duration_sec` - The length of the audio to generate in seconds.
    /// * `oscillators` - A relation with heading `(osc_id: Int, freq: Float, amp: Float, waveform: String)`.
    ///
    /// # Returns
    ///
    /// A relation with heading `(t: Float, sample: Float)` representing the mixed audio.
    pub fn generate(&self, duration_sec: f64, oscillators: &Relation) -> Result<Relation, String> {
        let num_samples = (duration_sec * self.sample_rate as f64) as usize;

        let time_heading = TupleType::new().with_attribute("t", ScalarType::Float);
        let mut time_rel = Relation::new(RelationType::new(time_heading));

        for i in 0..num_samples {
            let t = i as f64 / self.sample_rate as f64;
            time_rel
                .insert(tuple! { t: t })
                .map_err(|e| e.to_string())?;
        }

        // 1. Cartesian product of time steps and oscillators (join where they have no attributes in common)
        let joined = time_rel.join(oscillators).map_err(|e| e.to_string())?;

        // 2. Extend with computed sample value
        let with_samples = joined
            .extend("osc_sample", ScalarType::Float, |t: &Tuple| {
                let time = t.get_typed::<f64>("t").unwrap();
                let freq = t.get_typed::<f64>("freq").unwrap();
                let amp = t.get_typed::<f64>("amp").unwrap();
                let wave = t.get_typed::<String>("waveform").unwrap();

                let val = match wave.as_str() {
                    "sine" => amp * (2.0 * std::f64::consts::PI * freq * time).sin(),
                    "square" => amp * (2.0 * std::f64::consts::PI * freq * time).sin().signum(),
                    "sawtooth" => amp * 2.0 * (time * freq - (time * freq + 0.5).floor()),
                    _ => 0.0,
                };
                ScalarValue::Float(val)
            })
            .map_err(|e| e.to_string())?;

        // 3. Summarize (mix) by time
        let mixed = with_samples
            .summarize(&["t"], &[Aggregation::sum_float("sample", "osc_sample")])
            .map_err(|e| e.to_string())?;

        Ok(mixed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synth_generation() {
        let synth = Synth::new(100); // Low sample rate for test

        let osc_heading = TupleType::new()
            .with_attribute("osc_id", ScalarType::Int)
            .with_attribute("freq", ScalarType::Float)
            .with_attribute("amp", ScalarType::Float)
            .with_attribute("waveform", ScalarType::String);

        let mut oscillators = Relation::new(RelationType::new(osc_heading));
        oscillators
            .insert(tuple! { osc_id: 1i64, freq: 10.0, amp: 0.5, waveform: "sine" })
            .unwrap();
        oscillators
            .insert(tuple! { osc_id: 2i64, freq: 20.0, amp: 0.25, waveform: "square" })
            .unwrap();

        let audio = synth.generate(0.1, &oscillators).unwrap();

        assert_eq!(audio.cardinality(), 10);
        let degree = audio.degree();
        assert_eq!(degree, 2);
    }
}
