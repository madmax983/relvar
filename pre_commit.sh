#!/bin/bash
set -e

# Run format
cargo fmt --all

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test

# Run doc tests
cargo doc --no-deps
