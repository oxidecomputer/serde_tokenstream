// Copyright 2026 Oxide Computer Company

// Ensure that tokens left over after a ParseWrapper value are reported at the
// first stray token.

use testlib::annotation;

#[annotation {
    string = "test",
    options = OptionA,
    unit = (),
    tup = (1, 2.0),
    bool_expr = true false
}]
fn test() {}

fn main() {}
