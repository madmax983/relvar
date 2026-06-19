Let's prepare the plan to fix the Integer Overflow DoS in `image.rs`.

1. **Modify `apply_kernel` in `relvar/src/experimental/image.rs`**:
   - Replace `let total_weight: i64 = kernel.iter().map(|k| k.weight).sum();` with a saturating sum:
     `let total_weight: i64 = kernel.iter().fold(0i64, |acc, k| acc.saturating_add(k.weight));`

2. **Modify `compute_kernel_contributions` in `relvar/src/experimental/image.rs`**:
   - Replace `x + dx` with `x.saturating_add(dx)`
   - Replace `y + dy` with `y.saturating_add(dy)`
   - Replace `v * weight` with `v.saturating_mul(weight)` for all three channels (`r`, `g`, `b`).

3. **Modify `summarize_and_normalize` in `relvar/src/experimental/image.rs`**:
   - Replace `s / total_weight` with safe division: `s.checked_div(total_weight).unwrap_or(if s > 0 { i64::MAX } else { i64::MIN })`. Or better: `if total_weight == -1 && s == i64::MIN { i64::MAX } else { s / total_weight }`

4. **Verify changes**:
   - Write a test `warden_kernel_overflow.rs` (we already have it!). Let's run it.
   - It will pass instead of panic!

Wait, `warden.md` says:
"**Threat:** [The Vulnerability]
**Defense:** [The Fix]"
So I will write an entry in `.jules/warden.md`.

Let me request plan review.
