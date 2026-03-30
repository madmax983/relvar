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

process_file("relvar/src/experimental/raytracer.rs", [
    (
        "let mut scene = Scene::new();\n/// scene.add_sphere(0.0, 0.0, 5.0, 1.0, \"Red\");",
        "let mut scene = Scene::new();\n/// scene.add_sphere(0.0, 0.0, 5.0, 1.0, 255, 0, 0);"
    )
])
