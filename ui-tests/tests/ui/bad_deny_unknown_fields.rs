// Copyright 2026 Oxide Computer Company

// With `deny_unknown_fields`, an unknown field must be reported at the field
// name, not at the enclosing group.

use testlib::strict;

#[strict {
    a = 1,
    b = 2,
}]
fn test() {}

fn main() {}
