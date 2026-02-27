struct Wrapper(i32);

fn main() {
    let b = Box::new(Wrapper(42));
    println!("{}", b.0);
}
