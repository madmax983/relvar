Title: 🎻 Bard: [documentation update]

📖 Chapter: Documenting the `Spreadsheet` experimental module.
🔦 Insight: The spreadsheet implementation had placeholder examples for its struct and methods, making it hard for users to figure out how to structure values and evaluate formulas. Added clear, executable examples explaining the setup of values, formulas, and evaluation.
🧪 Example: Added 3 executable doctests. Also resolved visibility issues with internal modules breaking tests by adding #[doc(hidden)] and updating internal bench imports. Added dead code allowance for experimental files so we don't trigger hundreds of clippy warnings.
