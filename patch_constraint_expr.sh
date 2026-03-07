#!/bin/bash
cat << 'INNER_EOF' > relvar-core/src/constraints/expression.rs.patch
--- relvar-core/src/constraints/expression.rs
+++ relvar-core/src/constraints/expression.rs
@@ -28,6 +28,7 @@
 //! ```

 use super::prepared::PreparedConstraintExpression;
+use crate::utils::recursion::DepthGuarded;
 use crate::values::{ScalarValue, Tuple};
 use serde::{Deserialize, Serialize};
 use std::collections::HashSet;
@@ -87,7 +88,8 @@
 /// - **Logical**: And, Or, Not
 /// - **Set membership**: In
 /// - **Pattern matching**: Like
-#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
+#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
+#[serde(try_from = "ConstraintExpressionUnchecked")]
 pub enum ConstraintExpression {
     /// Comparison: left_attr op right_value_or_ref
     Cmp {
@@ -141,6 +143,62 @@
     /// ```
     Like(String, String),
 }
+
+// Private structure to assist with deserialization and validation.
+// This allows us to intercept deserialization and enforce invariants (MAX_TYPE_DEPTH)
+// that could be bypassed by serde if we derived Deserialize directly on ConstraintExpression.
+#[derive(Debug, Deserialize)]
+enum ConstraintExpressionUnchecked {
+    Cmp {
+        left: String,
+        op: CmpOp,
+        right: ValueOrRef,
+    },
+    And(
+        Box<DepthGuarded<ConstraintExpressionUnchecked>>,
+        Box<DepthGuarded<ConstraintExpressionUnchecked>>,
+    ),
+    Or(
+        Box<DepthGuarded<ConstraintExpressionUnchecked>>,
+        Box<DepthGuarded<ConstraintExpressionUnchecked>>,
+    ),
+    Not(Box<DepthGuarded<ConstraintExpressionUnchecked>>),
+    In(String, Vec<ScalarValue>),
+    Like(String, String),
+}
+
+impl TryFrom<ConstraintExpressionUnchecked> for ConstraintExpression {
+    type Error = String;
+
+    fn try_from(unchecked: ConstraintExpressionUnchecked) -> Result<Self, Self::Error> {
+        match unchecked {
+            ConstraintExpressionUnchecked::Cmp { left, op, right } => {
+                Ok(ConstraintExpression::Cmp { left, op, right })
+            }
+            ConstraintExpressionUnchecked::And(left, right) => {
+                let l = ConstraintExpression::try_from((*left).0)?;
+                let r = ConstraintExpression::try_from((*right).0)?;
+                Ok(ConstraintExpression::And(Box::new(l), Box::new(r)))
+            }
+            ConstraintExpressionUnchecked::Or(left, right) => {
+                let l = ConstraintExpression::try_from((*left).0)?;
+                let r = ConstraintExpression::try_from((*right).0)?;
+                Ok(ConstraintExpression::Or(Box::new(l), Box::new(r)))
+            }
+            ConstraintExpressionUnchecked::Not(inner) => {
+                let expr = ConstraintExpression::try_from((*inner).0)?;
+                Ok(ConstraintExpression::Not(Box::new(expr)))
+            }
+            ConstraintExpressionUnchecked::In(attr, values) => {
+                Ok(ConstraintExpression::In(attr, values))
+            }
+            ConstraintExpressionUnchecked::Like(attr, pattern) => {
+                Ok(ConstraintExpression::Like(attr, pattern))
+            }
+        }
+    }
+}

 impl ConstraintExpression {
     /// Scans the expression tree to collect all referenced attribute names.
INNER_EOF

patch relvar-core/src/constraints/expression.rs relvar-core/src/constraints/expression.rs.patch

cat << 'INNER_EOF' >> relvar-core/src/constraints/expression.rs

#[cfg(test)]
mod tests_coverage {
    use super::*;
    use crate::values::ScalarValue;

    #[test]
    fn test_constraint_expression_unchecked_try_from() {
        let unchecked = ConstraintExpressionUnchecked::Cmp {
            left: "test".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(1)),
        };
        let checked = ConstraintExpression::try_from(unchecked).unwrap();
        assert!(matches!(checked, ConstraintExpression::Cmp { .. }));
    }

    #[test]
    fn test_constraint_expression_unchecked_try_from_and() {
        let left = ConstraintExpressionUnchecked::Cmp {
            left: "test".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(1)),
        };
        let right = ConstraintExpressionUnchecked::Cmp {
            left: "test2".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(2)),
        };
        let unchecked = ConstraintExpressionUnchecked::And(
            Box::new(crate::utils::recursion::DepthGuarded(left)),
            Box::new(crate::utils::recursion::DepthGuarded(right)),
        );
        let checked = ConstraintExpression::try_from(unchecked).unwrap();
        assert!(matches!(checked, ConstraintExpression::And(..)));
    }

    #[test]
    fn test_constraint_expression_unchecked_try_from_or() {
        let left = ConstraintExpressionUnchecked::Cmp {
            left: "test".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(1)),
        };
        let right = ConstraintExpressionUnchecked::Cmp {
            left: "test2".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(2)),
        };
        let unchecked = ConstraintExpressionUnchecked::Or(
            Box::new(crate::utils::recursion::DepthGuarded(left)),
            Box::new(crate::utils::recursion::DepthGuarded(right)),
        );
        let checked = ConstraintExpression::try_from(unchecked).unwrap();
        assert!(matches!(checked, ConstraintExpression::Or(..)));
    }

    #[test]
    fn test_constraint_expression_unchecked_try_from_not() {
        let inner = ConstraintExpressionUnchecked::Cmp {
            left: "test".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(1)),
        };
        let unchecked = ConstraintExpressionUnchecked::Not(
            Box::new(crate::utils::recursion::DepthGuarded(inner)),
        );
        let checked = ConstraintExpression::try_from(unchecked).unwrap();
        assert!(matches!(checked, ConstraintExpression::Not(..)));
    }

    #[test]
    fn test_constraint_expression_unchecked_try_from_in() {
        let unchecked = ConstraintExpressionUnchecked::In(
            "test".to_string(),
            vec![ScalarValue::Int(1)],
        );
        let checked = ConstraintExpression::try_from(unchecked).unwrap();
        assert!(matches!(checked, ConstraintExpression::In(..)));
    }

    #[test]
    fn test_constraint_expression_unchecked_try_from_like() {
        let unchecked = ConstraintExpressionUnchecked::Like(
            "test".to_string(),
            "pattern".to_string(),
        );
        let checked = ConstraintExpression::try_from(unchecked).unwrap();
        assert!(matches!(checked, ConstraintExpression::Like(..)));
    }
}
INNER_EOF
