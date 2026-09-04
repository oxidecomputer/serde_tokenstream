// Copyright 2026 Oxide Computer Company

// Ensure that a top-level untagged enum with no matching variant, which serde
// reports only after the whole map has been consumed, produces an error
// rather than a panic.

use testlib::untagged;

#[untagged {
    c = 1,
}]
fn test() {}

fn main() {}
