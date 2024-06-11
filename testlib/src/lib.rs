// Copyright 2020 Oxide Computer Company

//! Simple proc macro consumer of `serde_tokenstream` that we use for testing
//! various failure cases.

use quote::ToTokens;
use serde::Deserialize;
use serde_tokenstream::from_tokenstream;
use serde_tokenstream::from_tokenstream_spanned;
use syn::parse_macro_input;

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

#[proc_macro_attribute]
pub fn outer(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = parse_macro_input!(item as syn::ItemFn);
    let annotation_attr =
        item.attrs.iter().find(|attr| attr.path().is_ident("annotation"));
    let annotation_attr = annotation_attr.expect("annotation attribute found");
    let l = match &annotation_attr.meta {
        syn::Meta::List(l) => l,
        _ => {
            panic!("annotation attribute must be a list")
        }
    };

    match from_tokenstream_spanned::<Annotation>(l.delimiter.span(), &l.tokens)
    {
        Ok(_) => {
            // Strip the annotation attribute from the function.
            let mut item = item.clone();
            item.attrs.retain(|attr| attr.path().is_ident("annotation"));
            item.into_token_stream().into()
        }
        Err(err) => err.to_compile_error().into(),
    }
}
