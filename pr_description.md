# 🪒 Razor: Essentialist cleanup of speculative experimental models

## Summary

In accordance with the KISS principle and the YAGNI protocol, this PR excises significant portions of dead, highly speculative, and over-engineered experimental code that modeled complex domain problems (such as Neural Networks, Blockchains, Game of Life, Raytracers, Audio Synthesizers) purely via relational algebra. These implementations provided no practical value to the core functionality and significantly inflated the maintenance burden.

### Changes made
- Removed dead/speculative files from `relvar/src/experimental/` and `relvar-core/src/experimental/`.
- Cleaned up respective `mod.rs` files to remove the module declarations.
- Removed dependent tests (e.g. `sentry_timeseries_collision.rs`, `warden_exploit_image_dos.rs`).
- Logged reductions in `.jules/razor.md` reflecting the rationale.

**Saved:** Hundreds of lines of purely speculative code and "Enterprise FizzBuzz" experiments.
