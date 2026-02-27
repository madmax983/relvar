use serde::Deserialize;
use std::cell::Cell;

thread_local! {
    static RECURSION_DEPTH: Cell<usize> = const { Cell::new(0) };
}

pub const MAX_RECURSION_DEPTH: usize = 64;

pub struct RecursionGuard;

impl RecursionGuard {
    pub fn new() -> Result<Self, &'static str> {
        RECURSION_DEPTH.with(|cell| {
            let depth = cell.get();
            if depth >= MAX_RECURSION_DEPTH {
                Err("Recursion limit exceeded")
            } else {
                cell.set(depth + 1);
                Ok(RecursionGuard)
            }
        })
    }
}

impl Drop for RecursionGuard {
    fn drop(&mut self) {
        RECURSION_DEPTH.with(|cell| {
            let depth = cell.get();
            if depth > 0 {
                cell.set(depth - 1);
            }
        });
    }
}

#[derive(Debug)]
pub struct DepthGuarded<T>(pub T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for DepthGuarded<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let _guard = RecursionGuard::new().map_err(serde::de::Error::custom)?;
        let value = T::deserialize(deserializer)?;
        Ok(DepthGuarded(value))
    }
}
