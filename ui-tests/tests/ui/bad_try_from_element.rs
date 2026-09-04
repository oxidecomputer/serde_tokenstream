// Copyright 2026 Oxide Computer Company

// A try_from failure in an array element must be attributed to the element,
// not the whole array.

use testlib::even_list;

#[even_list {
    items = [
        { n = 2 },
        { n = 3 },
    ],
}]
fn test() {}

fn main() {}
