import re

def process_file(filepath, replacements):
    with open(filepath, 'r') as f:
        content = f.read()

    for old, new in replacements:
        if old in content:
            content = content.replace(old, new)
        else:
            print(f"Warning: Could not find '{old}' in {filepath}")

    with open(filepath, 'w') as f:
        f.write(content)

process_file("relvar/src/experimental/image.rs", [
    (
        "let tap = KernelTap { dx: 0, dy: 0, weight: 1.0 };",
        "let tap = KernelTap { dx: 0, dy: 0, weight: 1 };"
    )
])

process_file("relvar/src/experimental/raytracer.rs", [
    (
        "let mut db = Database::new(InMemoryEngine::new());\n/// let mut scene = Scene::new(&mut db).unwrap();\n/// scene.add_sphere(0.0, 0.0, 5.0, 1.0, \"Red\").unwrap();",
        "let mut scene = Scene::new();\n/// scene.add_sphere(0.0, 0.0, 5.0, 1.0, \"Red\");"
    )
])
