// Copyright 2026 Oxide Computer Company

// Ensure that a top-level internally tagged enum with a missing tag, which
// serde reports only after the whole map has been consumed, produces an error
// rather than a panic.

use testlib::tagged;

#[tagged {
    x = 1,
}]
fn test() {}

fn main() {}
