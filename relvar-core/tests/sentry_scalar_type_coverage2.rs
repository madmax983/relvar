use relvar_core::types::ScalarType;
use std::cmp::Ordering;

#[test]
fn test_user_defined_cmp() {
    let t1 = ScalarType::user_defined("A", ScalarType::Int);
    let t2 = ScalarType::user_defined("B", ScalarType::Int);
    let t3 = ScalarType::user_defined("A", ScalarType::Float);
    let t4 = ScalarType::user_defined("A", ScalarType::Int);

    assert_eq!(t1.cmp(&t1), Ordering::Equal);
    assert_eq!(t1.cmp(&t2), Ordering::Less); // 'A' < 'B'
    assert_eq!(t2.cmp(&t1), Ordering::Greater); // 'B' > 'A'

    assert_eq!(t1.cmp(&t3), Ordering::Less); // Int < Float
    assert_eq!(t3.cmp(&t1), Ordering::Greater); // Float > Int

    assert_eq!(t1.cmp(&t4), Ordering::Equal); // A(Int) == A(Int)
}
