// Copyright 2026 Oxide Computer Company

// Ensure that an error from a `#[serde(flatten)]` field, which serde raises
// only after the whole map has been consumed, is correctly attributed.

use testlib::flattened;

#[flattened {
    outer = 1,
}]
fn test() {}

fn main() {}
