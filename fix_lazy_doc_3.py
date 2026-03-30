with open("relvar/src/experimental/raytracer.rs", 'r') as f:
    content = f.read()

content = content.replace("#[allow(clippy::too_many_arguments)]\n    /// Adds a sphere to the raytracing scene.\n///\n/// # Examples", "#[allow(clippy::too_many_arguments)]\n///\n/// # Examples")

with open("relvar/src/experimental/raytracer.rs", 'w') as f:
    f.write(content)
