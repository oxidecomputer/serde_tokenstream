// Copyright 2026 Oxide Computer Company

// Ensure that ParseWrapper over a compound struct with a hand-written Parse
// impl preserves span information from the inner syn::Error. The error for
// `"not a bool"` should point at that token, not at the surrounding groups.

use testlib::annotation;

#[annotation {
    string = "test",
    options = OptionA,
    unit = (),
    tup = (1, 2.0),
    painted = { color = { red = true, green = "not a bool", blue = false } }
}]
fn test() {}

fn main() {}
