// Copyright 2020 Oxide Computer Company

//! Simple proc macro consumer of `serde_tokenstream` that we use for testing
//! various failure cases.

use serde::Deserialize;
use serde_tokenstream::from_tokenstream;

#[derive(Deserialize)]
#[allow(dead_code)]
struct Annotation {
    string: String,
    maybe_string: Option<String>,
    options: Options,
    nested: Option<Nested>,
    many: Option<Vec<String>>,
    unit: (),
    tup: (u32, f32),
}

#[derive(Deserialize)]
#[allow(dead_code)]
enum Options {
    OptionA,
    OptionB,
    OptionC,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Nested {
    squeaker: String,
    eyas: u32,
    gosling: f64,
}

#[proc_macro_attribute]
pub fn annotation(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<Annotation>(&attr.into()) {
        Ok(_) => item,
        Err(err) => err.to_compile_error().into(),
    }
}
