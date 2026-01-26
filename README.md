# Relvar

A pure relational database management system (RDBMS) in Rust, built on the principles from C.J. Date and Hugh Darwen's work on relational theory, particularly "Database in Depth" and "The Third Manifesto".

## Overview

Relvar is an educational RDBMS that implements the relational model as it was originally intended by E.F. Codd and refined by C.J. Date and Hugh Darwen. The name "relvar" (short for "relation variable") is the standard term from Date and Darwen's work for a named, updatable relation in a database.

Unlike SQL databases, Relvar adheres strictly to relational theory:

- **No NULL values** - All attributes must have values
- **True set semantics** - Relations are sets of tuples (no duplicates, no ordering)
- **Proper type system** - Scalar types, tuple types, and relation types
- **Relation-valued attributes (RVAs)** - Relations can contain relations
- **Complete relational algebra** - All operators from relational theory

## Features

### ✅ Implemented (Phases 1-12 Complete)

- **Type System**
  - Scalar types: Int (i64), Float (f64), String, Bool, Bytes
  - Tuple types with named, typed attributes
  - Relation types (headings)
  - Relation-valued attributes (RVAs)
  - User-defined type constraints (Range, Enum, StringLength, etc.)

- **Relational Algebra**
  - Restrict (WHERE/filter)
  - Project (SELECT attributes)
  - Rename (attribute renaming)
  - Join (natural join, theta join)
  - Union, Intersection, Difference (set operations)
  - Extend (computed attributes)
  - Summarize (aggregation with grouping)
  - Group/Ungroup (RVA manipulation)

- **Constraints**
  - Candidate keys and primary keys
  - Foreign key constraints with referential integrity
  - Type constraints on scalar values
  - Constraint checking on all mutations

- **Storage Layer**
  - Page-based persistent storage (4KB pages)
  - Slotted page architecture for variable-length tuples
  - B-tree indexes for efficient lookups
  - System catalog for metadata
  - Heap files for tuple storage

- **Database API**
  - Create/drop relation variables (relvars)
  - Insert, delete, update operations
  - Query with full relational algebra
  - Transactions (begin/commit/rollback)

- **CI/CD**
  - Automated formatting checks (`cargo fmt`)
  - Linting with zero warnings (`cargo clippy`)
  - Cross-platform testing (Ubuntu, Windows, macOS)

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/relvar.git
cd relvar

# Build the project
cargo build --release

# Run tests
cargo test

# Run the demo
cargo run --example demo
```

## Quick Start

```rust
use relvar::{Database, RelationType, ScalarType, TupleType, tuple};
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a database
    let temp_dir = TempDir::new()?;
    let mut db = Database::open(temp_dir.path())?;

    // Define a relation type (heading)
    let employee_type = RelationType::new(
        TupleType::new()
            .with_attribute("emp_id".to_string(), ScalarType::Int)
            .with_attribute("name".to_string(), ScalarType::String)
            .with_attribute("dept_id".to_string(), ScalarType::Int),
    );

    // Create a relvar (relation variable)
    db.create_relvar("EMPLOYEE", employee_type)?;

    // Insert tuples
    db.insert("EMPLOYEE", tuple! {
        emp_id: 1i64,
        name: "Alice",
        dept_id: 10i64
    })?;

    db.insert("EMPLOYEE", tuple! {
        emp_id: 2i64,
        name: "Bob",
        dept_id: 20i64
    })?;

    db.insert("EMPLOYEE", tuple! {
        emp_id: 3i64,
        name: "Charlie",
        dept_id: 10i64
    })?;

    // Query using relational algebra
    let dept_10_employees = db.query("EMPLOYEE")?
        .restrict(|t| t.get_typed::<i64>("dept_id").unwrap() == 10)
        .project(&["emp_id", "name"]);

    println!("Department 10 employees:");
    for tuple in dept_10_employees.tuples() {
        println!("  {} - {}",
            tuple.get_typed::<i64>("emp_id").unwrap(),
            tuple.get_typed::<String>("name").unwrap()
        );
    }

    // Output:
    // Department 10 employees:
    //   1 - Alice
    //   3 - Charlie

    Ok(())
}
```

## Architecture

```
src/
├── lib.rs                 # Public API
├── types/                 # Type system
│   ├── scalar.rs          # Scalar types
│   ├── tuple_type.rs      # Tuple types
│   └── relation_type.rs   # Relation types
├── values/                # Value representations
│   ├── scalar.rs          # Scalar values
│   ├── tuple.rs           # Tuple values
│   └── relation.rs        # Relation values
├── algebra/               # Relational algebra operators
│   ├── restrict.rs        # Filter tuples
│   ├── project.rs         # Select attributes
│   ├── rename.rs          # Rename attributes
│   ├── join.rs            # Join operations
│   ├── union.rs           # Union
│   ├── intersect.rs       # Intersection
│   ├── difference.rs      # Set difference
│   ├── extend.rs          # Computed attributes
│   ├── summarize.rs       # Aggregation
│   └── group.rs           # GROUP/UNGROUP for RVAs
├── constraints/           # Constraint system
│   ├── type_constraint.rs # Type constraints
│   ├── key.rs             # Key constraints
│   └── foreign_key.rs     # Foreign keys
├── storage/               # Persistence layer
│   ├── page.rs            # Page-based storage
│   ├── heap.rs            # Heap files
│   ├── btree.rs           # B-tree indexes
│   └── catalog.rs         # System catalog
└── database.rs            # Main database API
```

## Key Principles

### No NULL Values

In the relational model, every attribute must have a value. There is no concept of "null" or "missing" data. This eliminates the three-valued logic problems that plague SQL.

```rust
// ✅ Valid - all attributes have values
let tuple = tuple! { id: 1i64, name: "Alice" };

// ❌ Not possible - cannot have missing attributes
// let tuple = tuple! { id: 1i64 };  // Missing 'name'
```

### True Set Semantics

Relations are mathematical sets of tuples. This means:
- No duplicate tuples
- No ordering of tuples
- No ordering of attributes within tuples

```rust
let mut relation = Relation::new(rel_type);
relation.insert(tuple1)?;
relation.insert(tuple1)?;  // Second insert has no effect (already in set)
assert_eq!(relation.cardinality(), 1);  // Only one tuple
```

### Strong Type System

Every value has a type, and operations are type-checked:

```rust
// Scalar types
let int_val = ScalarValue::Int(42);
let str_val = ScalarValue::String("hello".to_string());

// Tuple types
let person_type = TupleType::new()
    .with_attribute("id".to_string(), ScalarType::Int)
    .with_attribute("name".to_string(), ScalarType::String);

// Relation types
let people_type = RelationType::new(person_type);
```

### Relational Algebra

All operations are based on relational algebra, not SQL:

```rust
// Restrict (WHERE in SQL)
relation.restrict(|t| t.get_typed::<i64>("age").unwrap() > 18)

// Project (SELECT columns in SQL)
relation.project(&["id", "name"])

// Join (natural join)
employees.join(&departments)

// Union (requires type-compatible relations)
students.union(&teachers)
```

## Testing

The project follows Test-Driven Development (TDD):

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_heap_insert_and_read
```

## CI/CD

GitHub Actions automatically runs on every push:
- **fmt**: Code formatting check
- **clippy**: Linting with zero warnings
- **test**: Full test suite on Ubuntu, Windows, macOS
- **build**: Release build verification

## Roadmap

### Future Enhancements

- [ ] Query optimizer (cost-based optimization)
- [ ] Concurrency control (MVCC or 2PL)
- [ ] Write-Ahead Logging (WAL) for durability
- [ ] Query language parser (Tutorial D or custom syntax)
- [ ] Network protocol (client-server architecture)
- [ ] REPL (Read-Eval-Print Loop) for interactive use
- [ ] More aggregate functions (MEDIAN, MODE, etc.)
- [ ] Views (virtual relvars)
- [ ] Integrity constraints (domain constraints, assertions)
- [ ] Derived types (POSSREP/selector/observer pattern)

## References

- **"The Third Manifesto"** by C.J. Date and Hugh Darwen
  - The definitive formal specification of the relational model
  - A comprehensive proposal for the future of data and database management systems
  - [thethirdmanifesto.com](http://www.thethirdmanifesto.com/)

- **"Database in Depth: Relational Theory for Practitioners"** by C.J. Date (2005)
  - Accessible introduction to relational theory
  - A primary inspiration for this project
  - [O'Reilly Media](https://www.oreilly.com/library/view/database-in-depth/0596100124/)

- **"Databases, Types, and the Relational Model"** by C.J. Date and Hugh Darwen (3rd Edition)
  - Comprehensive treatment of type theory in databases
  - Foundation for proper relational type systems

- **"A Relational Model of Data for Large Shared Data Banks"** by E.F. Codd (1970)
  - The original paper that defined the relational model
  - [ACM Communications](https://dl.acm.org/doi/10.1145/362384.362685)

## Contributing

Contributions are welcome! Please:

1. Follow the existing code style (`cargo fmt`)
2. Ensure all tests pass (`cargo test`)
3. Add tests for new features
4. Keep clippy happy (`cargo clippy`)
5. Update documentation as needed

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- **C.J. Date and Hugh Darwen** for their tireless advocacy of the relational model and their collaborative work on The Third Manifesto, which provides the theoretical foundation for this project
- **E.F. Codd** for inventing the relational model and laying the groundwork for modern database theory
- The Rust community for excellent tooling and libraries

---

**Note**: This is an educational project implementing pure relational theory. For production use cases requiring SQL compatibility, consider PostgreSQL, SQLite, or other established databases.
