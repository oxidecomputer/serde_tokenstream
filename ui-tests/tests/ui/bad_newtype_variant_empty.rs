// Copyright 2026 Oxide Computer Company

// An empty newtype variant body must be an error with the span set to the
// parentheses, not a panic.

use testlib::newtype_variant;

#[newtype_variant {
    value = Named(),
}]
fn test() {}

fn main() {}
