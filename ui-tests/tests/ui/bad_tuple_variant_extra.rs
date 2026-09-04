// Copyright 2026 Oxide Computer Company

// Extra values in a tuple variant must produce an error.

use testlib::newtype_variant;

#[newtype_variant {
    value = Pair("a", "b", "c"),
}]
fn test() {}

fn main() {}
