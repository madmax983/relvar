import os

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

process_file("relvar/src/lib.rs", [
    (
        "pub fn open<P: AsRef<std::path::Path>>(",
        "/// Opens a persistent database from the given directory path.\n///\n/// This function initializes a `PersistentEngine` for robust storage\n/// and recovery features.\n///\n/// # Examples\n///\n/// ```ignore\n/// use relvar::open;\n/// let db = open(\"my_db\").unwrap();\n/// ```\n#[cfg(feature = \"storage\")]\npub fn open<P: AsRef<std::path::Path>>("
    )
])

process_file("relvar/src/experimental/image.rs", [
    (
        "pub struct KernelTap {",
        "/// Represents a weight at a specific offset for image processing convolutions.\n///\n/// # Examples\n///\n/// ```\n/// use relvar::experimental::image::KernelTap;\n/// let tap = KernelTap { dx: 0, dy: 0, weight: 1.0 };\n/// ```\npub struct KernelTap {"
    )
])

process_file("relvar/src/experimental/raytracer.rs", [
    (
        "pub fn add_sphere(",
        "/// Adds a sphere to the raytracing scene.\n///\n/// # Examples\n///\n/// ```\n/// use relvar::experimental::raytracer::Scene;\n/// use relvar::Database;\n/// use relvar::InMemoryEngine;\n/// let mut db = Database::new(InMemoryEngine::new());\n/// let mut scene = Scene::new(&mut db).unwrap();\n/// scene.add_sphere(0.0, 0.0, 5.0, 1.0, \"Red\").unwrap();\n/// ```\n    pub fn add_sphere("
    )
])

process_file("relvar/src/tools/exporter.rs", [
    (
        "pub enum ExporterError {",
        "/// Errors that can occur during data export.\n///\n/// These errors usually wrap underlying formatting or serialization issues.\n///\n/// # Examples\n///\n/// ```\n/// use relvar::tools::exporter::ExporterError;\n/// // Can be matched against underlying format errors.\n/// ```\npub enum ExporterError {"
    )
])

process_file("relvar/src/tools/importer.rs", [
    (
        "pub enum ImporterError {",
        "/// Errors that can occur during data import.\n///\n/// These typically result from malformed input data or schema mismatches.\n///\n/// # Examples\n///\n/// ```\n/// use relvar::tools::importer::ImporterError;\n/// // Allows matching specific parsing or conversion errors.\n/// ```\npub enum ImporterError {"
    )
])
