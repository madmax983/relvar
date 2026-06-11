1. **Refactor `compute_pairwise_forces` in `relvar/src/experimental/physics.rs`:**
   - The `compute_pairwise_forces` function is currently 81 lines long. It performs three main tasks: cross-joining the particles, filtering self-interactions, and calculating the forces for each pair in x and y components.
   - The force calculation logic for `fx` and `fy` is largely duplicated in the two `.extend()` calls.
   - I will apply the "Three-Phase Operator" pattern to extract logic out into well-named private helper functions.
   - I'll create a `calculate_pairwise_force_component` helper function to handle the core logic of calculating a specific force component, which can be called by `extend`. Wait, since `.extend` takes a closure, maybe it's better to just extract the creation of the interactions relation into a helper and the force calculation into another. Or I can extract the closure logic into a helper method `calculate_force_component(t: &Tuple, g: f64, is_x: bool) -> ScalarValue`.
   - Actually, a simpler way is:
     ```rust
     fn cross_join_particles(&self) -> Result<Relation, DatabaseError>
     fn calculate_forces(&self, interactions: Relation) -> Result<Relation, DatabaseError>
     ```
     But `compute_pairwise_forces` *is* calculating forces.
     Let's look at `calculate_pairwise_force_component` or `compute_force_component`:
     ```rust
     fn compute_force_component(t: &Tuple, g: f64, component: &str) -> ScalarValue {
         let x1 = t.get_typed::<f64>("x1").unwrap();
         // ...
         let dx = x2 - x1;
         let dy = y2 - y1;
         let dist_sq = dx * dx + dy * dy;
         if dist_sq < 1e-10 { return ScalarValue::Float(0.0); }
         let dist = dist_sq.sqrt();
         let f = g * m1 * m2 / dist_sq;
         if component == "x" {
             ScalarValue::Float(f * (dx / dist))
         } else {
             ScalarValue::Float(f * (dy / dist))
         }
     }
     ```
   - Let's read `compute_pairwise_forces` again to see what is best.
   - Wait, `compute_pairwise_forces` has 3 distinct steps: 1) Cross join, 2) Filter self-interactions, 3) Compute forces.
   - Let's extract:
     1. `generate_particle_pairs(&self) -> Result<Relation, DatabaseError>`
     2. `compute_force_component(t: &Tuple, g: f64, is_x: bool) -> ScalarValue`
   - Actually, `compute_force_component` takes `&Tuple` directly, resolving the "God Function" issue as mentioned in the memories for `relvar-core/src/experimental/...`.
   - The Forge journal states: `Avoid embedding complex mathematical or logical operations directly within .extend() closures in relvar and relvar-core. Refactor these 'God Functions' by extracting the logic into well-named private helper functions that take a &Tuple argument to improve readability and flatten structure.`

2. **Add a journal entry to `.jules/forge.md`:**
   - I'll document that I extracted the force calculation from `compute_pairwise_forces` using a helper function taking a `&Tuple` to avoid large inline `.extend()` closures.

3. **Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done:**
   - Run `cargo fmt --all`
   - Run `cargo clippy --all-targets --all-features -- -D warnings`
   - Run `cargo test`

4. **Submit the PR:**
   - Create `pr_description.md` and run `pr.py` via `run_in_bash_session`.
