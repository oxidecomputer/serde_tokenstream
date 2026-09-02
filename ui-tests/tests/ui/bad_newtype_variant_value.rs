// Copyright 2026 Oxide Computer Company

// An error from inside a newtype variant must be attributed to the value, not
// to the parentheses.

use testlib::newtype_variant;

#[newtype_variant {
    value = Even({ n = 3 }),
}]
fn test() {}

fn main() {}
