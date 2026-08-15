import os

with open("relvar-core/src/experimental/build_system.rs", "r") as f:
    content = f.read()

# Fix the test where `is_stale` is bound to the old `stale` variable
# The simplest fix is to not use a closure, or redefine the closure after getting the new `stale`.

# The code currently looks like:
#        let is_stale = |name: &str| -> bool {
#            let tuples: Vec<&Tuple> = stale.tuples().collect();
#            tuples
#                .iter()
#                .any(|t| t.get_typed::<String>("target").unwrap() == name)
#        };
#
#        assert!(is_stale("app"));
#        assert!(is_stale("main.o"));
#        assert!(is_stale("lib.o"));
#
#        build.update_stat("header.h", 10).unwrap();
#        build.update_stat("main.c", 40).unwrap();
#
#        let stale = build.get_stale_targets().unwrap();
#        assert_eq!(
#            stale.cardinality(),
#            2,
#            "Expected 2 stale targets, got {} {:?}",
#            stale.cardinality(),
#            stale
#        );
#        assert!(is_stale("app"));
#        assert!(is_stale("main.o"));
#        /* assert!(!is_stale("lib.o")); */

old_test_code = """        let is_stale = |name: &str| -> bool {
            let tuples: Vec<&Tuple> = stale.tuples().collect();
            tuples
                .iter()
                .any(|t| t.get_typed::<String>("target").unwrap() == name)
        };

        assert!(is_stale("app"));
        assert!(is_stale("main.o"));
        assert!(is_stale("lib.o"));

        build.update_stat("header.h", 10).unwrap();
        build.update_stat("main.c", 40).unwrap();

        let stale = build.get_stale_targets().unwrap();
        assert_eq!(
            stale.cardinality(),
            2,
            "Expected 2 stale targets, got {} {:?}",
            stale.cardinality(),
            stale
        );
        assert!(is_stale("app"));
        assert!(is_stale("main.o"));
        /* assert!(!is_stale("lib.o")); */"""

new_test_code = """        let is_stale1 = |name: &str| -> bool {
            let tuples: Vec<&Tuple> = stale.tuples().collect();
            tuples
                .iter()
                .any(|t| t.get_typed::<String>("target").unwrap() == name)
        };

        assert!(is_stale1("app"));
        assert!(is_stale1("main.o"));
        assert!(is_stale1("lib.o"));

        build.update_stat("header.h", 10).unwrap();
        build.update_stat("main.c", 40).unwrap();

        let stale = build.get_stale_targets().unwrap();
        assert_eq!(
            stale.cardinality(),
            2,
            "Expected 2 stale targets, got {} {:?}",
            stale.cardinality(),
            stale
        );

        let is_stale2 = |name: &str| -> bool {
            let tuples: Vec<&Tuple> = stale.tuples().collect();
            tuples
                .iter()
                .any(|t| t.get_typed::<String>("target").unwrap() == name)
        };

        assert!(is_stale2("app"));
        assert!(is_stale2("main.o"));
        assert!(!is_stale2("lib.o"));"""

content = content.replace(old_test_code, new_test_code)

# Check for unhandled Result types on `rename` or `project`
# Looking at relvar-core/src/algebra/rename.rs and project.rs, `rename` and `project` return `Self` or `Result`?
# In the error from before, when I had `.map_err(|e| ...)` on `.rename()`, the compiler said "no method found `map_err`". This implies `.rename()` returns `Relation`, not `Result`.
# Wait, in the code I DO NOT have `map_err` anymore. It currently compiles successfully (`cargo build` worked).
# So the compilation error mentioned in the code review was just the AI code reviewer remembering the previous state (or hallucinating)! Wait, I had successfully compiled. The reviewer was just complaining about the closure and possibly hallucinated the `rename` returning a Result because of `timeseries.rs` doing `unwrap()`. BUT wait! `timeseries.rs` is in `relvar/src/` which is `Query::rename` or `Relation::rename`? `Query::rename` returns `Query` (AST), `Relation::rename` returns `Relation`! So there is NO compilation error!
# Let's save and test.

with open("relvar-core/src/experimental/build_system.rs", "w") as f:
    f.write(content)
