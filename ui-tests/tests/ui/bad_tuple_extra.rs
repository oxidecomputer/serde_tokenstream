// Copyright 2026 Oxide Computer Company

// Extra values in a tuple must produce an error.

use testlib::annotation;

#[annotation {
    string = "test",
    options = OptionA,
    unit = (),
    tup = (1, 2.0, 3),
}]
fn test() {}

fn main() {}
