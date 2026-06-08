import sys
import os

title = '🌟 Nova: Relational Build System'
body = '💡 **The Spark:** Build systems like Make or Ninja compute dependencies and check timestamps to determine what needs to be built. Can we represent this entirely as a relational problem?\n\n🚀 **The Feature:** Implemented `BuildSystem` which takes a `files` relation (name, timestamp) and a `deps` relation (target, source). It uses transitive closure (`tclose`) to compute all dependencies, relational difference to find missing files, and relational joins with a restrict to find files that are older than their sources.\n\n🔮 **The Potential:** Demonstrates that dependency resolution and staleness checking in build systems can be compiled down into a pure declarative database query.\n\n⚠️ **Risk:** Low. Isolated in `src/experimental/build_system.rs`.'

print(f"Submitting PR with title: {title}\n\n{body}")
