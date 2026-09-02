// Copyright 2026 Oxide Computer Company

// Ensure that a value parsed out of a SpannedString carries the span of the
// value: the macro emits a use of the parsed identifier, and rustc reports
// the unresolved name at the string literal in the attribute.

use testlib::annotation;

#[annotation {
    string = "test",
    options = OptionA,
    unit = (),
    tup = (1, 2.0),
    ident = "undefined_thing",
}]
fn test() {}

fn main() {}
