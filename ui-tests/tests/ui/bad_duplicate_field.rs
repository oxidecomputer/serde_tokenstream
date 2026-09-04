// Copyright 2026 Oxide Computer Company

// serde raises duplicate-field errors between next_key_seed and
// next_value_seed, so the best available span is the enclosing group.
// Ensure that this is what's produced.

use testlib::strict;

#[strict {
    a = 1,
    a = 2,
}]
fn test() {}

fn main() {}
