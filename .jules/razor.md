## [Reduction]
**Bloat:** Empty `ImageProcessor` and `Tokenizer` structs acting purely as namespaces for static methods (Enterprise FizzBuzz "God Struct" style).
**Cut:** Removed the empty structs and `impl` blocks, changing the static methods into module-level free functions. Updated usage sites.
**Saved:** Reduced boilerplate, improved clarity, explicit module-level function calls instead of Object-Oriented faux namespaces.
