// Copyright 2026 Oxide Computer Company

// Ensure that a SpannedString value that doesn't even lex as Rust tokens is
// reported at the value.

use testlib::annotation;

#[annotation {
    string = "test",
    options = OptionA,
    unit = (),
    tup = (1, 2.0),
    ident = "\"",
}]
fn test() {}

fn main() {}
