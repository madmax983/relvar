use std::hash::{Hash, Hasher};

#[derive(Debug)]
enum MyEnum {
    Int,
    Float,
}

impl Hash for MyEnum {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            MyEnum::Int => {}
            MyEnum::Float => {}
        }
    }
}
fn main() {}
