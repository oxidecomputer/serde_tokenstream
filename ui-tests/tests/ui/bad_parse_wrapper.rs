// Copyright 2024 Oxide Computer Company

// Ensure that a bad value in ParseWrapper (in this case a type mismatch) gets
// the correct span information.

use testlib::annotation;

#[annotation {
    string = "test",
    options = OptionA,
    unit = (),
    tup = (1, 2.0),
    bool_expr = 2 + 2
}]
fn test() {}

fn main() {}
