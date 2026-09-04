// Copyright 2026 Oxide Computer Company

// Ensure that SpannedString composes inside a hand-written Parse type: the
// value is parsed in the middle of the token stream, and an error about it
// is reported at the value rather than at the separator or the attribute.

use testlib::annotation;

#[annotation {
    string = "test",
    options = OptionA,
    unit = (),
    tup = (1, 2.0),
    pair = foo: "not an identifier",
}]
fn test() {}

fn main() {}
