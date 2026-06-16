git show HEAD:.jules/razor.md > tmp.md
cat << 'INNER_EOF' >> tmp.md

## 2025-02-28 - [Reduction]
**Bloat:** Duplicated experimental feature `KnowledgeGraph` between `relvar` and `relvar-core`.
**Cut:** Deleted `relvar/src/experimental/knowledge_graph.rs` and retained the robust version in `relvar-core`.
**Saved:** 363 lines of code / 1 Duplicate concept.
INNER_EOF
mv tmp.md .jules/razor.md
