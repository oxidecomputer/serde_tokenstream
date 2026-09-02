// Copyright 2026 Oxide Computer Company

// Ensure that a ParseWrapper inside a `#[serde(flatten)]` struct produces a
// helpful error message.

use testlib::flattened_wrapper;

#[flattened_wrapper {
    n = 1,
    id = foo,
}]
fn test() {}

fn main() {}
