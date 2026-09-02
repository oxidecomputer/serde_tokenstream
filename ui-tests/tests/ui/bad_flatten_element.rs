// Copyright 2026 Oxide Computer Company

// Ensure that an error from a `#[serde(flatten)]` field inside an array
// element is attributed to that element rather than to the whole array.

use testlib::flattened_list;

#[flattened_list {
    items = [
        { outer = 1, needed = 2 },
        { outer = 3 },
    ],
}]
fn test() {}

fn main() {}
