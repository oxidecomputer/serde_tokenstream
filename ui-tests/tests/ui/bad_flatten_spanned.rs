// Copyright 2026 Oxide Computer Company

// Ensure that an error from a `#[serde(flatten)]` field at the top level of a
// `from_tokenstream_spanned` call is attributed to the braces rather than to
// the whole attribute.

use testlib::flattened_outer;

#[flattened_outer]
#[flattened {
    outer = 1,
}]
fn test() {}

fn main() {}
