use relvar::experimental::image::{apply_kernel, KernelTap, load};
use relvar_core::values::Relation;

fn main() {
    let width = 2;
    let height = 2;
    let data: Vec<u8> = vec![
        255, 0, 0, // Red
        0, 255, 0, // Green
        0, 0, 255, // Blue
        255, 255, 255, // White
    ];

    let relation = load(width, height, &data);

    let kernel = vec![
        KernelTap { dx: 1, dy: 1, weight: i64::MAX },
        KernelTap { dx: 0, dy: 0, weight: i64::MAX },
    ];

    apply_kernel(&relation, &kernel);
}
