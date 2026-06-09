import os
from pr_agent import submit_pr

submit_pr(
    title='⚒️ Forge: Refactor Spreadsheet Evaluate Function',
    description='🚮 Smell: The `evaluate` method in `relvar/src/experimental/spreadsheet.rs` was a "God Function". It combined finding unresolved formulas, preparing mathematical arguments, evaluating operations, and handling result union all inside a single large loop block, hurting readability.\n\n✨ Solution: Extracted the logic into three descriptive helper functions: `find_unresolved_formulas`, `prepare_arguments`, and `evaluate_formulas`.\n\n🧼 Benefit: Dramatically improves readability by applying the Three-Phase Operator pattern. The main loop is now very short, flat, and its phases are clearly defined.\n\n🛡️ Verification: `cargo test` passes. Behavior is unchanged as this was purely an extraction refactoring.'
)
