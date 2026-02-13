use relvar::constraints::{CmpOp, ConstraintExpression, ValueOrRef};
use relvar::experimental::query::{Query, QueryAggregation, QueryAggregationFn};
use relvar::types::{RelationType, ScalarType, TupleType};
use relvar::values::ScalarValue;
use relvar::{tuple, Database, InMemoryEngine};

#[test]
fn test_query_builder_e2e() {
    // 1. Setup Database
    let mut db = Database::new(InMemoryEngine::new());

    // Create EMPLOYEES
    let emp_heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("name", ScalarType::String)
        .with_attribute("dept_id", ScalarType::Int)
        .with_attribute("salary", ScalarType::Int);

    db.create_relvar("EMPLOYEES", RelationType::new(emp_heading))
        .unwrap();

    db.insert(
        "EMPLOYEES",
        tuple! { id: 1i64, name: "Alice", dept_id: 10i64, salary: 50000i64 },
    )
    .unwrap();
    db.insert(
        "EMPLOYEES",
        tuple! { id: 2i64, name: "Bob", dept_id: 20i64, salary: 60000i64 },
    )
    .unwrap();
    db.insert(
        "EMPLOYEES",
        tuple! { id: 3i64, name: "Charlie", dept_id: 10i64, salary: 55000i64 },
    )
    .unwrap();

    // Create DEPARTMENTS
    let dept_heading = TupleType::new()
        .with_attribute("dept_id", ScalarType::Int)
        .with_attribute("dept_name", ScalarType::String);

    db.create_relvar("DEPARTMENTS", RelationType::new(dept_heading))
        .unwrap();

    db.insert(
        "DEPARTMENTS",
        tuple! { dept_id: 10i64, dept_name: "Engineering" },
    )
    .unwrap();
    db.insert(
        "DEPARTMENTS",
        tuple! { dept_id: 20i64, dept_name: "Sales" },
    )
    .unwrap();

    // 2. Build Query
    // Query: Get Engineering employees (dept_id=10), show name and salary
    let query = Query::scan("EMPLOYEES")
        .restrict(ConstraintExpression::Cmp {
            left: "dept_id".to_string(),
            op: CmpOp::Eq,
            right: ValueOrRef::Value(ScalarValue::Int(10)),
        })
        .project(vec!["name", "salary"]);

    // 3. Execute
    let result = query.execute(&mut db).unwrap();

    assert_eq!(result.cardinality(), 2);
    // Alice and Charlie

    // 4. Explain
    let explanation = query.explain();
    println!("Explanation:\n{}", explanation);
    assert!(explanation.contains("Scan(EMPLOYEES)"));
    assert!(explanation.contains("Restrict"));
    assert!(explanation.contains("Project"));

    // 5. Serialize / Deserialize
    let json = serde_json::to_string(&query).unwrap();
    println!("JSON:\n{}", json);
    let loaded: Query = serde_json::from_str(&json).unwrap();

    let result_loaded = loaded.execute(&mut db).unwrap();
    assert_eq!(result_loaded.cardinality(), 2);
}

#[test]
fn test_query_join_summarize() {
    let mut db = Database::new(InMemoryEngine::new());

    // Create EMPLOYEES
    let emp_heading = TupleType::new()
        .with_attribute("id", ScalarType::Int)
        .with_attribute("dept_id", ScalarType::Int)
        .with_attribute("salary", ScalarType::Int);

    db.create_relvar("EMPLOYEES", RelationType::new(emp_heading))
        .unwrap();
    db.insert(
        "EMPLOYEES",
        tuple! { id: 1i64, dept_id: 10i64, salary: 50000i64 },
    )
    .unwrap();
    db.insert(
        "EMPLOYEES",
        tuple! { id: 2i64, dept_id: 10i64, salary: 60000i64 },
    )
    .unwrap();
    db.insert(
        "EMPLOYEES",
        tuple! { id: 3i64, dept_id: 20i64, salary: 70000i64 },
    )
    .unwrap();

    // Create DEPARTMENTS
    let dept_heading = TupleType::new()
        .with_attribute("dept_id", ScalarType::Int)
        .with_attribute("dept_name", ScalarType::String);

    db.create_relvar("DEPARTMENTS", RelationType::new(dept_heading))
        .unwrap();
    db.insert(
        "DEPARTMENTS",
        tuple! { dept_id: 10i64, dept_name: "Engineering" },
    )
    .unwrap();
    db.insert(
        "DEPARTMENTS",
        tuple! { dept_id: 20i64, dept_name: "Sales" },
    )
    .unwrap();

    // Query: Join Emp + Dept, Summarize by Dept Name, Count and Avg Salary
    let query = Query::scan("EMPLOYEES")
        .join(Query::scan("DEPARTMENTS"))
        .summarize(
            vec!["dept_name"],
            vec![
                QueryAggregation {
                    result_name: "count".to_string(),
                    result_type: ScalarType::Int,
                    function: QueryAggregationFn::Count,
                },
                QueryAggregation {
                    result_name: "avg_salary".to_string(),
                    result_type: ScalarType::Float,
                    function: QueryAggregationFn::Avg("salary".to_string()),
                },
            ],
        );

    let result = query.execute(&mut db).unwrap();

    assert_eq!(result.cardinality(), 2);

    // Verify Engineering (dept_id 10) -> count 2, avg 55000
    let eng_tuple = result
        .tuples()
        .find(|t| t.get_typed::<String>("dept_name").unwrap() == "Engineering")
        .unwrap();

    assert_eq!(eng_tuple.get_typed::<i64>("count").unwrap(), 2);
    assert_eq!(eng_tuple.get_typed::<f64>("avg_salary").unwrap(), 55000.0);
}
