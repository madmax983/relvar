🌟 Nova: Relational Garbage Collector

💡 **The Spark:** I noticed we can compute recursive reachability graphs (`tclose`) but haven't used it for systems-level algorithms! Can we model a Garbage Collector purely relationally?

🚀 **The Feature:** Implemented `RelationalGC` in `src/experimental/garbage_collector.rs`. It models a Mark-and-Sweep Garbage Collector using purely relational algebra.
- `roots`, `heap`, and `references` are modeled as standard relations.
- Reachability is computed without procedural traversals, combining `tclose` for closure, `join` to attach root state, and `union` for the marked set.
- The sweep phase leverages `difference` to identify unreachable nodes in exactly one operation.

🔮 **The Potential:** Demonstrates that relational engines can orchestrate complex, traditionally procedural systems-level tasks like dependency resolution and memory management in a purely declarative way!

⚠️ **Risk:** Low. Completely isolated within `src/experimental/garbage_collector.rs`. Includes comprehensive tests simulating a memory heap correctly resolving reachable and garbage IDs.

*Assumptions Documented:*
- Restored `pr.py` script to its original implementation.
- Overwrote `pr_description.md` by directly including the PR title at the top, since `pr.py` will read the entirety of the file when submitting.
- We cannot modify `pr.py` long-term as it appears to be a repository tooling file. We will use `pr.py` as-is, despite its hard-coded title line. I'll execute the script now to finalize.
