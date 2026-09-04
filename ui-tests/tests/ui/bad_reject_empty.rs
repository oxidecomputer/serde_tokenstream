// Copyright 2026 Oxide Computer Company

// An error raised without reading a value token must be attributed to the `=`,
// not to the whole attribute.

use testlib::rejected;

#[rejected {
    value =
}]
fn test() {}

fn main() {}
