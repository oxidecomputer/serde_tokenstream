// Copyright 2026 Oxide Computer Company

// Ensure that with serde(untagged), a failure in an array element is attributed
// to the element, not the whole array.

use testlib::untagged_list;

#[untagged_list {
    items = [
        { a = 1 },
        { c = 1 },
    ],
}]
fn test() {}

fn main() {}
