use relvar_core::{
    algebra::Aggregation,
    tuple,
    types::{RelationType, ScalarType, TupleType},
    values::{Relation, ScalarValue},
};

/// A Relational Petri Net Simulator.
///
/// In a Petri Net:
/// - Places hold discrete numbers of tokens.
/// - Transitions connect to places via input and output arcs.
/// - A transition is 'enabled' if all its input places have at least the required token weight.
/// - Firing a transition removes tokens from its input places and adds tokens to its output places.
///
/// We model this using purely relational algebra.
///
/// # Examples
///
/// ```
/// use relvar::experimental::petri_net::PetriNet;
///
/// let mut net = PetriNet::new();
/// net.add_place("p_start", 1);
/// net.add_transition("t_run");
/// net.add_input_arc("p_start", "t_run", 1);
///
/// // The transition is enabled because p_start has 1 token
/// assert!(net.fire("t_run"));
/// ```
pub struct PetriNet {
    /// Schema: { place_id: String, tokens: Int }
    pub places: Relation,
    /// Schema: { transition_id: String }
    pub transitions: Relation,
    /// Schema: { transition_id: String, place_id: String, weight: Int }
    pub input_arcs: Relation,
    /// Schema: { transition_id: String, place_id: String, weight: Int }
    pub output_arcs: Relation,
}

impl PetriNet {
    /// Creates a new, empty Petri Net.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::experimental::petri_net::PetriNet;
    ///
    /// let net = PetriNet::new();
    /// assert_eq!(net.places.cardinality(), 0);
    /// assert_eq!(net.transitions.cardinality(), 0);
    /// ```
    pub fn new() -> Self {
        let places_type = RelationType::new(
            TupleType::new()
                .with_attribute("place_id", ScalarType::String)
                .with_attribute("tokens", ScalarType::Int),
        );
        let transitions_type =
            RelationType::new(TupleType::new().with_attribute("transition_id", ScalarType::String));
        let arcs_type = RelationType::new(
            TupleType::new()
                .with_attribute("transition_id", ScalarType::String)
                .with_attribute("place_id", ScalarType::String)
                .with_attribute("weight", ScalarType::Int),
        );

        Self {
            places: Relation::new(places_type),
            transitions: Relation::new(transitions_type.clone()),
            input_arcs: Relation::new(arcs_type.clone()),
            output_arcs: Relation::new(arcs_type),
        }
    }

    /// Adds a place with an initial token count.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::experimental::petri_net::PetriNet;
    ///
    /// let mut net = PetriNet::new();
    /// net.add_place("p1", 5);
    /// assert_eq!(net.places.cardinality(), 1);
    /// ```
    pub fn add_place(&mut self, id: &str, tokens: i64) {
        self.places
            .insert(tuple! { place_id: id.to_string(), tokens: tokens })
            .unwrap();
    }

    /// Adds a transition.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::experimental::petri_net::PetriNet;
    ///
    /// let mut net = PetriNet::new();
    /// net.add_transition("t1");
    /// assert_eq!(net.transitions.cardinality(), 1);
    /// ```
    pub fn add_transition(&mut self, id: &str) {
        self.transitions
            .insert(tuple! { transition_id: id.to_string() })
            .unwrap();
    }

    /// Adds an input arc from a place to a transition.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::experimental::petri_net::PetriNet;
    ///
    /// let mut net = PetriNet::new();
    /// net.add_place("p1", 1);
    /// net.add_transition("t1");
    /// net.add_input_arc("p1", "t1", 1);
    /// assert_eq!(net.input_arcs.cardinality(), 1);
    /// ```
    pub fn add_input_arc(&mut self, place_id: &str, transition_id: &str, weight: i64) {
        self.input_arcs
            .insert(tuple! { transition_id: transition_id.to_string(), place_id: place_id.to_string(), weight: weight })
            .unwrap();
    }

    /// Adds an output arc from a transition to a place.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::experimental::petri_net::PetriNet;
    ///
    /// let mut net = PetriNet::new();
    /// net.add_transition("t1");
    /// net.add_place("p2", 0);
    /// net.add_output_arc("t1", "p2", 1);
    /// assert_eq!(net.output_arcs.cardinality(), 1);
    /// ```
    pub fn add_output_arc(&mut self, transition_id: &str, place_id: &str, weight: i64) {
        self.output_arcs
            .insert(tuple! { transition_id: transition_id.to_string(), place_id: place_id.to_string(), weight: weight })
            .unwrap();
    }

    /// Returns a relation of enabled transitions.
    /// Schema: { transition_id: String }
    ///
    /// A transition is enabled if FOR ALL input arcs, the connected place has tokens >= weight.
    /// We do this relationally:
    /// 1. Join `input_arcs` with `places`.
    /// 2. Find "missing" requirements by checking where `tokens < weight`.
    /// 3. Project to `transition_id` to get disabled transitions.
    /// 4. Difference `transitions` - `disabled_transitions` = `enabled_transitions`.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::experimental::petri_net::PetriNet;
    /// use relvar_core::tuple;
    ///
    /// let mut net = PetriNet::new();
    /// net.add_place("p1", 1);
    /// net.add_transition("t1");
    /// net.add_input_arc("p1", "t1", 1);
    ///
    /// let enabled = net.enabled_transitions();
    /// assert_eq!(enabled.cardinality(), 1);
    /// assert!(enabled.contains(&tuple! { transition_id: "t1".to_string() }));
    /// ```
    pub fn enabled_transitions(&self) -> Relation {
        // Find requirements
        let requirements = self.input_arcs.join(&self.places).unwrap();

        // Find failing requirements: tokens < weight
        let failing = requirements.restrict(|t| {
            let tokens = t.get_typed::<i64>("tokens").unwrap();
            let weight = t.get_typed::<i64>("weight").unwrap();
            tokens < weight
        });

        // Get transitions that have at least one failing requirement
        let disabled_transitions = failing.project(&["transition_id"]);

        // Enabled = All - Disabled
        self.transitions.difference(&disabled_transitions).unwrap()
    }

    /// Fires a transition by its ID, updating the token counts in `places`.
    /// Returns true if it successfully fired, false if it wasn't enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use relvar::experimental::petri_net::PetriNet;
    /// use relvar_core::tuple;
    ///
    /// let mut net = PetriNet::new();
    /// net.add_place("p1", 1);
    /// net.add_place("p2", 0);
    /// net.add_transition("t1");
    /// net.add_input_arc("p1", "t1", 1);
    /// net.add_output_arc("t1", "p2", 1);
    ///
    /// // Fire successfully
    /// assert!(net.fire("t1"));
    ///
    /// // Try to fire again (now p1 has 0 tokens, so t1 is not enabled)
    /// assert!(!net.fire("t1"));
    /// ```
    pub fn fire(&mut self, transition_id: &str) -> bool {
        let enabled = self.enabled_transitions();
        let query_tuple = tuple! { transition_id: transition_id.to_string() };
        if !enabled.contains(&query_tuple) {
            return false;
        }

        let this_transition_rel =
            Relation::from_tuples(self.transitions.relation_type().clone(), vec![query_tuple])
                .unwrap();

        let aggregated_deltas = self.compute_aggregated_deltas(&this_transition_rel);
        self.apply_deltas_to_places(&aggregated_deltas);

        true
    }

    fn compute_aggregated_deltas(&self, transition_rel: &Relation) -> Relation {
        let consumed = self.input_arcs.join(transition_rel).unwrap();
        let produced = self.output_arcs.join(transition_rel).unwrap();

        let consumed_deltas = consumed
            .extend("delta", ScalarType::Int, |t| {
                let weight = t.get_typed::<i64>("weight").unwrap();
                ScalarValue::Int(-weight)
            })
            .unwrap()
            .project(&["place_id", "delta"]);

        let produced_deltas = produced
            .extend("delta", ScalarType::Int, |t| {
                let weight = t.get_typed::<i64>("weight").unwrap();
                ScalarValue::Int(weight)
            })
            .unwrap()
            .project(&["place_id", "delta"]);

        let all_deltas = consumed_deltas.union(&produced_deltas).unwrap();

        all_deltas
            .summarize(&["place_id"], &[Aggregation::sum("net_delta", "delta")])
            .unwrap()
    }

    fn apply_deltas_to_places(&mut self, aggregated_deltas: &Relation) {
        let joined = self.places.join(aggregated_deltas).unwrap();
        let modified_places = joined
            .extend("new_tokens", ScalarType::Int, |t| {
                let tokens = t.get_typed::<i64>("tokens").unwrap();
                let net_delta = t.get_typed::<i64>("net_delta").unwrap();
                ScalarValue::Int(tokens + net_delta)
            })
            .unwrap()
            .project(&["place_id", "new_tokens"])
            .rename(&[("new_tokens", "tokens")]);

        let modified_place_ids = modified_places.project(&["place_id"]);
        let unmodified_places = self.places.semidifference(&modified_place_ids);

        self.places = modified_places.union(&unmodified_places).unwrap();
    }
}

impl Default for PetriNet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_petri_net_basic_flow() {
        let mut net = PetriNet::new();

        // Simple producer-consumer model
        net.add_place("p_ready", 1);
        net.add_place("p_working", 0);
        net.add_place("p_done", 0);

        net.add_transition("t_start");
        net.add_transition("t_finish");

        // t_start: p_ready (1) -> p_working (1)
        net.add_input_arc("p_ready", "t_start", 1);
        net.add_output_arc("t_start", "p_working", 1);

        // t_finish: p_working (1) -> p_done (1)
        net.add_input_arc("p_working", "t_finish", 1);
        net.add_output_arc("t_finish", "p_done", 1);

        // Check enabled: only t_start should be enabled initially
        let enabled = net.enabled_transitions();
        assert_eq!(enabled.cardinality(), 1);
        assert!(enabled.contains(&tuple! { transition_id: "t_start".to_string() }));

        // Fire t_start
        assert!(net.fire("t_start"));

        // Now t_start is disabled, t_finish is enabled
        let enabled2 = net.enabled_transitions();
        assert_eq!(enabled2.cardinality(), 1);
        assert!(enabled2.contains(&tuple! { transition_id: "t_finish".to_string() }));

        // Fire t_finish
        assert!(net.fire("t_finish"));

        // Network is deadlocked (all done)
        let enabled3 = net.enabled_transitions();
        assert_eq!(enabled3.cardinality(), 0);

        // Check final token state
        let done_place = net
            .places
            .restrict(|t| t.get_typed::<String>("place_id").unwrap() == "p_done");
        let done_tuple = done_place.tuples().next().unwrap();
        assert_eq!(done_tuple.get_typed::<i64>("tokens").unwrap(), 1);
    }

    #[test]
    fn test_petri_net_multiple_inputs() {
        let mut net = PetriNet::new();

        net.add_place("hydrogen", 2);
        net.add_place("oxygen", 1);
        net.add_place("water", 0);

        net.add_transition("synthesize");

        net.add_input_arc("hydrogen", "synthesize", 2);
        net.add_input_arc("oxygen", "synthesize", 1);
        net.add_output_arc("synthesize", "water", 1);

        assert!(net.fire("synthesize"));

        let water = net
            .places
            .restrict(|t| t.get_typed::<String>("place_id").unwrap() == "water");
        assert_eq!(
            water
                .tuples()
                .next()
                .unwrap()
                .get_typed::<i64>("tokens")
                .unwrap(),
            1
        );

        let h = net
            .places
            .restrict(|t| t.get_typed::<String>("place_id").unwrap() == "hydrogen");
        assert_eq!(
            h.tuples()
                .next()
                .unwrap()
                .get_typed::<i64>("tokens")
                .unwrap(),
            0
        );
    }
}
