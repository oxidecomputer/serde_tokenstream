// Copyright 2026 Oxide Computer Company

// A ParseWrapper value that is missing at the end of the input must be reported
// as a missing value at the `=`.

use testlib::annotation;

#[annotation {
    string = "test",
    options = OptionA,
    unit = (),
    tup = (1, 2.0),
    bool_expr =
}]
fn test() {}

fn main() {}
