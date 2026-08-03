#![allow(dead_code)]
//! Relational Enigma Machine
//!
//! This module models an Enigma machine using purely relational algebra.
//! The plugboard, rotors, and reflector are represented as relations mapping
//! input pins to output pins. Encryption is performed in parallel over an
//! entire message relation using `Join` to simulate electrical pathways and
//! `Extend` to calculate rotational offsets.
//!
//! # Concepts
//!
//! - **Message**: A relation containing `(pos: Int, char: Int)` where `char` is 0-25.
//! - **Plugboard**: A relation `(pin_in: Int, pin_out: Int)` acting as a bijection.
//! - **Rotors**: Relations `(pos: Int, pin_in: Int, pin_out: Int)` for each rotor step.
//! - **Reflector**: A relation `(pin_in: Int, pin_out: Int)`.

use relvar_core::{
    error::DatabaseError,
    types::ScalarType,
    values::{Relation, ScalarValue},
};

/// A Relational Enigma Machine.
/// # Examples
///
/// ```
/// use relvar::{Relation, RelationType, ScalarType, TupleType};
/// use relvar::experimental::enigma::EnigmaMachine;
/// // Note: This is a placeholder example
/// ```
pub struct EnigmaMachine {
    /// The plugboard mapping. Schema: (pin_in: Int, pin_out: Int)
    pub plugboard: Relation,
    /// Rotor 1 mapping. Schema: (pin_in: Int, pin_out: Int)
    pub rotor1: Relation,
    /// Rotor 2 mapping. Schema: (pin_in: Int, pin_out: Int)
    pub rotor2: Relation,
    /// Rotor 3 mapping. Schema: (pin_in: Int, pin_out: Int)
    pub rotor3: Relation,
    /// Reflector mapping. Schema: (pin_in: Int, pin_out: Int)
    pub reflector: Relation,
}

impl EnigmaMachine {
    /// Creates a new Enigma machine from its constituent relational components.
    /// # Examples
    ///
    /// ```
    /// use relvar_core::types::{TupleType, RelationType, ScalarType};
    /// use relvar_core::values::Relation;
    /// use relvar::experimental::enigma::EnigmaMachine;
    ///
    /// let rel_type = RelationType::new(TupleType::new().with_attribute("pin_in", ScalarType::Int).with_attribute("pin_out", ScalarType::Int));
    /// let empty = Relation::new(rel_type);
    /// let enigma = EnigmaMachine::new(empty.clone(), empty.clone(), empty.clone(), empty.clone(), empty.clone());
    /// ```
    pub fn new(
        plugboard: Relation,
        rotor1: Relation,
        rotor2: Relation,
        rotor3: Relation,
        reflector: Relation,
    ) -> Self {
        Self {
            plugboard,
            rotor1,
            rotor2,
            rotor3,
            reflector,
        }
    }

    fn apply_static_mapping(rel: &Relation, mapping: &Relation) -> Result<Relation, DatabaseError> {
        Ok(rel
            .rename(&[("char", "pin_in")])
            .join(mapping)?
            .project(&["pos", "pin_out"])
            .rename(&[("pin_out", "char")]))
    }

    fn pass_forward(
        rel: Relation,
        rotor: &Relation,
        step_divisor: i64,
    ) -> Result<Relation, DatabaseError> {
        let offset_rel = rel
            .extend("offset", ScalarType::Int, move |t| {
                let pos = t.get_typed::<i64>("pos").unwrap();
                ScalarValue::Int((pos / step_divisor) % 26)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let enter_rel = offset_rel
            .extend("pin_in", ScalarType::Int, |t| {
                let c = t.get_typed::<i64>("char").unwrap();
                let offset = t.get_typed::<i64>("offset").unwrap();
                ScalarValue::Int((c + offset).rem_euclid(26))
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let joined = enter_rel
            .project(&["pos", "pin_in", "offset"])
            .join(rotor)?;

        Ok(joined
            .extend("out_char", ScalarType::Int, |t| {
                let pin_out = t.get_typed::<i64>("pin_out").unwrap();
                let offset = t.get_typed::<i64>("offset").unwrap();
                ScalarValue::Int((pin_out - offset).rem_euclid(26))
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["pos", "out_char"])
            .rename(&[("out_char", "char")]))
    }

    fn pass_backward(
        rel: Relation,
        rotor: &Relation,
        step_divisor: i64,
    ) -> Result<Relation, DatabaseError> {
        let offset_rel = rel
            .extend("offset", ScalarType::Int, move |t| {
                let pos = t.get_typed::<i64>("pos").unwrap();
                ScalarValue::Int((pos / step_divisor) % 26)
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let enter_rel = offset_rel
            .extend("pin_out", ScalarType::Int, |t| {
                let c = t.get_typed::<i64>("char").unwrap();
                let offset = t.get_typed::<i64>("offset").unwrap();
                ScalarValue::Int((c + offset).rem_euclid(26))
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?;

        let joined = enter_rel
            .project(&["pos", "pin_out", "offset"])
            .join(rotor)?;

        Ok(joined
            .extend("out_char", ScalarType::Int, |t| {
                let pin_in = t.get_typed::<i64>("pin_in").unwrap();
                let offset = t.get_typed::<i64>("offset").unwrap();
                ScalarValue::Int((pin_in - offset).rem_euclid(26))
            })
            .map_err(|e| DatabaseError::AlgebraError(e.to_string()))?
            .project(&["pos", "out_char"])
            .rename(&[("out_char", "char")]))
    }

    /// Encrypts (or decrypts) a message relation.
    ///
    /// The message should have the schema `(pos: Int, char: Int)`.
    /// Returns a relation with the same schema containing the encrypted characters.
    /// # Examples
    ///
    /// ```text
    /// // Encrypt relation
    /// ```
    pub fn encrypt(&self, message: &Relation) -> Result<Relation, DatabaseError> {
        // We evaluate the electrical path for the entire message in parallel!
        //
        // Forward path:
        // message -> plugboard -> rotor1 -> rotor2 -> rotor3 -> reflector
        //
        // Return path:
        // reflector -> rotor3 (inverse) -> rotor2 (inverse) -> rotor1 (inverse) -> plugboard -> output

        // 1. Pass through plugboard
        let mut current = Self::apply_static_mapping(message, &self.plugboard)?;

        // 2. Forward through rotors
        current = Self::pass_forward(current, &self.rotor1, 1)?;
        current = Self::pass_forward(current, &self.rotor2, 26)?;
        current = Self::pass_forward(current, &self.rotor3, 676)?;

        // 3. Pass through reflector
        current = Self::apply_static_mapping(&current, &self.reflector)?;

        // 4. Backward through rotors
        current = Self::pass_backward(current, &self.rotor3, 676)?;
        current = Self::pass_backward(current, &self.rotor2, 26)?;
        current = Self::pass_backward(current, &self.rotor1, 1)?;

        // 5. Backward through plugboard
        current = Self::apply_static_mapping(&current, &self.plugboard)?;

        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use relvar_core::{
        tuple,
        types::{RelationType, TupleType},
    };

    // Helper to build a generic wiring relation (pin_in -> pin_out)
    fn build_wiring(wiring_func: impl Fn(i64) -> i64) -> Relation {
        let heading = TupleType::new()
            .with_attribute("pin_in", ScalarType::Int)
            .with_attribute("pin_out", ScalarType::Int);
        let mut rel = Relation::new(RelationType::new(heading));

        for i in 0..26 {
            rel.insert(tuple! {
                pin_in: i,
                pin_out: wiring_func(i),
            })
            .unwrap();
        }
        rel
    }

    fn build_rotor(offset: i64) -> Relation {
        // A simple bijection for testing: (i * 3 + offset) % 26
        // To be a bijection, the multiplier must be coprime with 26.
        // Coprimes of 26: 1, 3, 5, 7, 9, 11, 15, 17, 19, 21, 23, 25.
        build_wiring(|i| (i * 3 + offset) % 26)
    }

    fn build_reflector() -> Relation {
        // Reflector must be an involution: f(f(x)) = x and f(x) != x
        // A simple one is (x + 13) % 26
        build_wiring(|i| (i + 13) % 26)
    }

    fn build_plugboard() -> Relation {
        // Simple involution for plugboard (swaps adjacent pairs: 0<->1, 2<->3, etc.)
        build_wiring(|i| if i % 2 == 0 { i + 1 } else { i - 1 })
    }

    #[test]
    fn test_enigma_encryption_and_decryption() {
        let machine = EnigmaMachine::new(
            build_plugboard(),
            build_rotor(1),
            build_rotor(2),
            build_rotor(3),
            build_reflector(),
        );

        let heading = TupleType::new()
            .with_attribute("pos", ScalarType::Int)
            .with_attribute("char", ScalarType::Int);
        let mut message = Relation::new(RelationType::new(heading));

        // Create a message: [10, 11, 12]
        message.insert(tuple! { pos: 0i64, char: 10i64 }).unwrap();
        message.insert(tuple! { pos: 1i64, char: 11i64 }).unwrap();
        message.insert(tuple! { pos: 2i64, char: 12i64 }).unwrap();

        let ciphertext = machine.encrypt(&message).unwrap();

        // Enigma ciphertext should never be the same as plaintext
        let is_different = ciphertext.tuples().any(|t| {
            let pos = t.get_typed::<i64>("pos").unwrap();
            let c_char = t.get_typed::<i64>("char").unwrap();
            let p_char = pos + 10;
            c_char != p_char
        });
        assert!(is_different, "Ciphertext should differ from plaintext");

        // Enigma is symmetric: encrypting the ciphertext yields the plaintext
        let decrypted = machine.encrypt(&ciphertext).unwrap();

        assert_eq!(decrypted.cardinality(), 3);
        for t in decrypted.tuples() {
            let pos = t.get_typed::<i64>("pos").unwrap();
            let char_val = t.get_typed::<i64>("char").unwrap();
            assert_eq!(
                char_val,
                pos + 10,
                "Decrypted message must match original at pos {}",
                pos
            );
        }
    }
}
