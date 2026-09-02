// Copyright 2026 Oxide Computer Company

// Ensure that an error about a SpannedString value is reported at the value
// rather than at the attribute as a whole.

use testlib::annotation;

#[annotation {
    string = "test",
    options = OptionA,
    unit = (),
    tup = (1, 2.0),
    ident = "not an identifier",
}]
fn test() {}

fn main() {}
