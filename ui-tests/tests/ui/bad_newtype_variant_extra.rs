// Copyright 2026 Oxide Computer Company

// Extra values in a newtype variant must produce an error.

use testlib::newtype_variant;

#[newtype_variant {
    value = Named("a", "b"),
}]
fn test() {}

fn main() {}
