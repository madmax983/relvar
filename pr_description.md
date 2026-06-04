🌟 Nova: Relational CA Sequencer

💡 **The Spark:** "We have a fully functional Cellular Automaton (`cellular_automaton`) and a Relational Audio Synthesizer (`synth`). Can we make the Game of Life sing by treating it as a step sequencer?"
🚀 **The Feature:** "Implemented `ca_sequencer::CaSequencer`. For a given number of steps, it advances a cellular automaton generation by generation. Alive cells are mapped via `Extend` to oscillators (pitch is based on the y-coordinate) and fed into the `Synth`. The resulting audio snippet is accumulated via `Union` into a master timeline."
🔮 **The Potential:** "Demonstrates that relational algebra can orchestrate complex real-time algorithmic music generation by passing states between two separate relational engines!"
⚠️ **Risk:** "Low. Purely additive feature contained entirely in `src/experimental/ca_sequencer.rs`."
