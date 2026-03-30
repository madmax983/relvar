import re

with open("relvar-core/src/database/integrity.rs", "r") as f:
    content = f.read()

# Fix the duplicated lines in get_key_constraints docs
content = content.replace(
"""    /// let current_constraints = db.get_key_constraints("TEST").unwrap();
    /// assert!(current_constraints.primary_key().is_some());
    /// ```
    /// assert!(current_constraints.primary_key().is_some());
    /// ```""",
"""    /// let current_constraints = db.get_key_constraints("TEST").unwrap();
    /// assert!(current_constraints.primary_key().is_some());
    /// ```"""
)

with open("relvar-core/src/database/integrity.rs", "w") as f:
    f.write(content)
