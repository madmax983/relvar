//! Relational Audio Synthesis (RAS)
//!
//! This module demonstrates digital signal processing using pure relational algebra.
//! Audio signals are modeled as relations of `(sample_idx: Int, amplitude: Float)`.
//!
//! Operations like mixing become simple `Union` + `Summarize` (Sum) operations.
//! Envelopes become `Join` + `Extend` (Multiplication).

use relvar_core::algebra::summarize::Aggregation;
use relvar_core::tuple;
use relvar_core::types::{RelationType, ScalarType, TupleType};
use relvar_core::values::{Relation, ScalarValue};

/// Tools for generating and processing audio as relations.
pub struct AudioProcessor;

impl AudioProcessor {
    /// Generates a sine wave as a relation.
    ///
    /// The resulting relation has heading `(sample_idx: Int, amplitude: Float)`.
    ///
    /// # Arguments
    ///
    /// * `frequency` - Frequency in Hz (e.g., 440.0).
    /// * `duration_seconds` - Duration in seconds.
    /// * `sample_rate` - Samples per second (e.g., 44100).
    pub fn sine_wave(frequency: f64, duration_seconds: f64, sample_rate: usize) -> Relation {
        let heading = TupleType::new()
            .with_attribute("sample_idx", ScalarType::Int)
            .with_attribute("amplitude", ScalarType::Float);

        let rel_type = RelationType::new(heading);
        let mut relation = Relation::new(rel_type);

        let total_samples = (duration_seconds * sample_rate as f64) as usize;

        // We iterate and insert.
        // Note: For large durations, this is slow.
        for i in 0..total_samples {
            let t = i as f64;
            // sin(2 * PI * f * t) where t is in seconds.
            // t_seconds = i / sample_rate
            // angular_freq = 2 * PI * frequency
            // value = sin(angular_freq * t_seconds)
            let value = (2.0 * std::f64::consts::PI * frequency * (t / sample_rate as f64)).sin();

            relation
                .insert(tuple! {
                    sample_idx: i as i64,
                    amplitude: value
                })
                .unwrap();
        }

        relation
    }

    /// Mixes multiple audio tracks together.
    ///
    /// This is equivalent to summing the amplitudes at each time step.
    /// Implemented as:
    /// 1. Extend each track with a unique `track_id` to preserve duplicates.
    /// 2. `Union` all tracks.
    /// 3. `Summarize` by `sample_idx`, summing `amplitude`.
    pub fn mix(tracks: &[&Relation]) -> Relation {
        if tracks.is_empty() {
            let heading = TupleType::new()
                .with_attribute("sample_idx", ScalarType::Int)
                .with_attribute("amplitude", ScalarType::Float);
            return Relation::new(RelationType::new(heading));
        }

        // 1. Prepare contributions with unique track IDs
        // We need to make tuples distinct across tracks so Union preserves them.
        let mut contributions = Vec::new();

        for (i, track) in tracks.iter().enumerate() {
            let track_id = i as i64;
            // Add track_id to make tuples unique across tracks
            // We use a closure that captures track_id
            let extended = track
                .extend("track_id", ScalarType::Int, move |_| {
                    ScalarValue::Int(track_id)
                })
                .unwrap();
            contributions.push(extended);
        }

        // 2. Union all extended tracks
        let mut unioned = contributions[0].clone();
        for other in contributions.iter().skip(1) {
            unioned = unioned.union(other).unwrap();
        }

        // 3. Summarize (Group By sample_idx, Sum amplitude)
        // Grouping by sample_idx will aggregate across different track_ids
        let summarized = unioned
            .summarize(
                &["sample_idx"],
                &[Aggregation::sum_float("mixed_amplitude", "amplitude")],
            )
            .unwrap();

        // 4. Rename back to standard schema (sample_idx, amplitude)
        // summarize result is (sample_idx, mixed_amplitude)
        summarized.rename(&[("mixed_amplitude", "amplitude")])
    }

    /// Converts an audio relation to a raw sample buffer (f32).
    ///
    /// The buffer is sorted by `sample_idx`.
    pub fn to_buffer(relation: &Relation) -> Vec<f32> {
        // We need to sort by sample_idx to ensure correct playback order.
        let mut samples: Vec<(i64, f32)> = relation
            .tuples()
            .map(|t| {
                let idx = t.get_typed::<i64>("sample_idx").unwrap_or(0);
                let amp = t.get_typed::<f64>("amplitude").unwrap_or(0.0) as f32;
                (idx, amp)
            })
            .collect();

        samples.sort_by_key(|k| k.0);

        samples.into_iter().map(|(_, amp)| amp).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_wave_generation() {
        let freq = 440.0;
        let duration = 0.01; // 10ms
        let rate = 44100;

        let wave = AudioProcessor::sine_wave(freq, duration, rate);

        let expected_samples = (duration * rate as f64) as usize; // 441
        assert_eq!(wave.cardinality(), expected_samples);

        // Check first sample (t=0) -> sin(0) = 0
        let buffer = AudioProcessor::to_buffer(&wave);
        assert!((buffer[0] - 0.0).abs() < 1e-6);

        // Check quarter cycle (peak)
        // period = 44100 / 440 = ~100.22 samples
        // peak at ~25 samples
        let peak_idx = 25;
        assert!(buffer[peak_idx] > 0.9);
    }

    #[test]
    fn test_mix_constructive_interference() {
        let freq = 440.0;
        let duration = 0.01;
        let rate = 44100;

        let wave1 = AudioProcessor::sine_wave(freq, duration, rate);
        let wave2 = AudioProcessor::sine_wave(freq, duration, rate); // Identical

        let mixed = AudioProcessor::mix(&[&wave1, &wave2]);

        assert_eq!(mixed.cardinality(), wave1.cardinality());

        let buf1 = AudioProcessor::to_buffer(&wave1);
        let buf_mixed = AudioProcessor::to_buffer(&mixed);

        for (v1, vm) in buf1.iter().zip(buf_mixed.iter()) {
            // Mixed should be exactly double
            assert!(
                (vm - (v1 * 2.0)).abs() < 1e-6,
                "Expected {} to be 2x {}",
                vm,
                v1
            );
        }
    }

    #[test]
    fn test_mix_destructive_interference() {
        // Create an inverted wave manually
        let freq = 440.0;
        let duration = 0.01;
        let rate = 44100;

        let wave1 = AudioProcessor::sine_wave(freq, duration, rate);

        // Invert wave1
        let wave2 = wave1
            .extend("inv_amp", ScalarType::Float, |t| {
                let a = t.get_typed::<f64>("amplitude").unwrap();
                ScalarValue::Float(-a)
            })
            .unwrap()
            .project(&["sample_idx", "inv_amp"])
            .rename(&[("inv_amp", "amplitude")]);

        let mixed = AudioProcessor::mix(&[&wave1, &wave2]);

        let buf_mixed = AudioProcessor::to_buffer(&mixed);

        for vm in buf_mixed.iter() {
            // Should be near zero
            assert!(vm.abs() < 1e-6);
        }
    }

    #[test]
    fn test_to_buffer_ordering() {
        let heading = TupleType::new()
            .with_attribute("sample_idx", ScalarType::Int)
            .with_attribute("amplitude", ScalarType::Float);
        let mut rel = Relation::new(RelationType::new(heading));

        // Insert in random order
        rel.insert(tuple! { sample_idx: 2i64, amplitude: 0.2 })
            .unwrap();
        rel.insert(tuple! { sample_idx: 0i64, amplitude: 0.0 })
            .unwrap();
        rel.insert(tuple! { sample_idx: 1i64, amplitude: 0.1 })
            .unwrap();

        let buffer = AudioProcessor::to_buffer(&rel);

        assert_eq!(buffer, vec![0.0, 0.1, 0.2]);
    }
}
