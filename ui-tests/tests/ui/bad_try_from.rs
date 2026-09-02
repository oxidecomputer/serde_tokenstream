// Copyright 2026 Oxide Computer Company

// A top-level try_from failure must result in an error, not a panic. Because we
// don't have a span, the error will (unfortunately) be attributed to the entire
// attribute.

use testlib::even;

#[even {
    n = 3,
}]
fn test() {}

fn main() {}
