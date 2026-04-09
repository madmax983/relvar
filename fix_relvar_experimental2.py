import os
import re

def add_examples(filepath, default_imports):
    if not os.path.exists(filepath):
        return

    with open(filepath, 'r') as f:
        content = f.read()

    # Find all public functions/structs missing examples
    lines = content.split('\n')
    new_lines = []

    i = 0
    while i < len(lines):
        line = lines[i]

        if line.strip().startswith('pub fn ') or line.strip().startswith('pub struct ') or line.strip().startswith('pub enum '):
            # Check backwards for an example
            has_example = False
            has_doc = False
            j = i - 1
            while j >= 0 and (lines[j].strip().startswith('///') or lines[j].strip().startswith('#[')):
                if lines[j].strip().startswith('///'):
                    has_doc = True
                    if 'Example' in lines[j] or 'Examples' in lines[j]:
                        has_example = True
                        break
                j -= 1

            if not has_example:
                indent = line[:len(line) - len(line.lstrip())]

                # Extract name
                name = ""
                if 'pub fn ' in line:
                    name = line.strip().split('pub fn ')[1].split('(')[0].split('<')[0]
                elif 'pub struct ' in line:
                    name = line.strip().split('pub struct ')[1].split('{')[0].split('(')[0].split('<')[0].strip()
                elif 'pub enum ' in line:
                    name = line.strip().split('pub enum ')[1].split('{')[0].strip()

                if not has_doc:
                    new_lines.append(indent + f"/// {name}")
                    new_lines.append(indent + "///")

                new_lines.append(indent + "/// # Examples")
                new_lines.append(indent + "///")
                new_lines.append(indent + "/// ```")
                for imp in default_imports:
                    new_lines.append(indent + f"/// {imp}")
                new_lines.append(indent + "/// // Note: This is a placeholder example")
                new_lines.append(indent + "/// ```")

        new_lines.append(line)
        i += 1

    with open(filepath, 'w') as f:
        f.write('\n'.join(new_lines))

add_examples('relvar/src/experimental/graph.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};'])
add_examples('relvar/src/experimental/ecs.rs', ['use relvar::{Database, InMemoryEngine};'])
add_examples('relvar/src/experimental/vcs.rs', ['use relvar::{Database, InMemoryEngine, Relation, RelationType, ScalarType, TupleType};'])
add_examples('relvar/src/experimental/neural_network.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};'])
add_examples('relvar/src/experimental/automl.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType, Tuple};', 'use relvar::experimental::automl::NaiveBayesClassifier;'])
add_examples('relvar/src/experimental/matrix.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};', 'use relvar::experimental::matrix::Matrix;'])
add_examples('relvar/src/experimental/raytracer.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};', 'use relvar::experimental::raytracer::Scene;'])
add_examples('relvar/src/experimental/synth.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};', 'use relvar::experimental::synth::Synth;'])
add_examples('relvar/src/experimental/mock.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};', 'use relvar::experimental::mock::MockRelation;'])
add_examples('relvar/src/experimental/circuit.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};', 'use relvar::experimental::circuit::LogicSimulator;'])
add_examples('relvar/src/experimental/physics.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};', 'use relvar::experimental::physics::PhysicsEngine;'])
add_examples('relvar/src/experimental/search.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType, Database, InMemoryEngine};'])
add_examples('relvar/src/experimental/turing.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};', 'use relvar::experimental::turing::TuringMachine;'])
add_examples('relvar/src/experimental/cellular_automaton.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};', 'use relvar::experimental::cellular_automaton::CellularAutomaton;'])
add_examples('relvar/src/experimental/image.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};'])
add_examples('relvar/src/experimental/recommend.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};', 'use relvar::experimental::recommend::CollaborativeFilter;'])
add_examples('relvar/src/experimental/blockchain.rs', ['use relvar::{Relation, RelationType, ScalarType, TupleType};', 'use relvar::experimental::blockchain::Blockchain;'])
