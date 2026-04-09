import re
import os

files_to_fix = [
    "relvar/src/experimental/vcs.rs",
    "relvar/src/experimental/ecs.rs",
    "relvar/src/experimental/cellular_automaton.rs",
    "relvar/src/experimental/image.rs",
    "relvar/src/experimental/recommend.rs",
    "relvar/src/experimental/synth.rs",
    "relvar/src/experimental/blockchain.rs",
    "relvar/src/experimental/matrix.rs",
    "relvar/src/experimental/physics.rs",
    "relvar/src/experimental/search.rs",
    "relvar/src/experimental/turing.rs",
    "relvar/src/experimental/raytracer.rs",
    "relvar/src/experimental/graph.rs",
    "relvar/src/experimental/circuit.rs",
    "relvar/src/experimental/mock.rs",
    "relvar/src/experimental/neural_network.rs",
    "relvar/src/experimental/automl.rs",
]

for file in files_to_fix:
    if os.path.exists(file):
        with open(file, 'r') as f:
            lines = f.readlines()

        # Simple test to check file contents before actually writing
        print(f"Read {file}")
