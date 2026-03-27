import os

files_to_check = [
    './relvar-storage/src/persistent_engine.rs',
    './relvar-core/src/values/tuple.rs',
    './relvar-core/src/database/schema.rs',
    './relvar-core/src/database/data.rs',
    './relvar-core/src/storage_engine/mod.rs',
    './relvar-core/src/utils/recursion.rs',
    './relvar-core/src/algebra/divide.rs',
    './relvar-core/src/algebra/delta.rs',
    './relvar-core/src/query/mod.rs',
    './relvar/src/experimental/ecs.rs',
    './relvar/src/experimental/recommend.rs',
    './relvar/src/experimental/search.rs',
    './relvar/src/experimental/raytracer.rs',
    './relvar/src/experimental/graph.rs'
]

for path in files_to_check:
    with open(path, 'r') as fp:
        content = fp.read()

    # Let's fix these to be even better, as instructed
    content = content.replace('/// Computes and returns an error', '/// Yields an error')
    content = content.replace('/// Computes and returns a Serde error', '/// Yields a Serde error')
    content = content.replace('/// Computes and returns a `DatabaseError`', '/// Yields a `DatabaseError`')
    content = content.replace('/// Computes and returns all tuples', '/// Derives all tuples')
    content = content.replace('/// Computes and returns a new `Delta`', '/// Generates a new `Delta`')
    content = content.replace('/// Computes and returns a human-readable', '/// Produces a human-readable')
    content = content.replace('/// Computes and returns a relation', '/// Constructs a relation')

    content = content.replace('/// Retrieves the', '/// Obtains the')
    content = content.replace('/// Retrieves a', '/// Obtains a')

    with open(path, 'w') as fp:
        fp.write(content)
