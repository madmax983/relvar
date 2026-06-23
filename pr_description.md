⚒️ Forge: Refactored calculate_visible_pixels into smaller helpers

🚮 Smell: `calculate_visible_pixels` was a 68-line God function mixing hit filtering, color mapping, and background combining.
✨ Solution: Applied the Three-Phase Operator pattern by extracting `filter_hits`, `map_colors`, and `combine_with_background`.
🧼 Benefit: Improved readability and separation of concerns.
🛡️ Verification: Tests passed. No logic changed.
