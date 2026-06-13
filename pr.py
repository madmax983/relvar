import sys
import os

print(f"""Submitting PR with title: ⚒️ Forge: Extract God Function in Spreadsheet Evaluation

🚮 Smell: The `evaluate` method in `relvar/src/experimental/spreadsheet.rs` was a 66-line "God Function" combining finding unresolved formulas, resolving arguments, and evaluating formulas into a single continuous block, violating the "Three-Phase Operator" pattern.
✨ Solution: Extracted the logic into three cleanly typed private helper functions: `find_unresolved_formulas`, `resolve_arguments`, and `evaluate_formulas`.
🧼 Benefit: Flattens the main method and significantly increases code readability without sacrificing logic or safety.
🛡️ Verification: Tests passed. No logic changed.""")
