// Copyright 2026 Oxide Computer Company

// Ensure that a SpannedString value written as a bare identifier (here, a
// keyword) is reported at the identifier.

use testlib::annotation;

#[annotation {
    string = "test",
    options = OptionA,
    unit = (),
    tup = (1, 2.0),
    ident = mod,
}]
fn test() {}

fn main() {}
