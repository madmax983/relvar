## 2024-05-18 - ConstraintExpression::In HashSet optimization
**Learning:** Pre-computing HashSet at constraint expression construction time is significantly faster than computing it per-tuple at evaluation time.
**Action:** Changed the ConstraintExpression::In variant to directly hold a std::collections::HashSet instead of a Vec. This allowed evaluate_in to drop its O(N) or runtime allocation logic and instead do an O(1) direct lookup, boosting performance significantly (-98.8% time taken).
