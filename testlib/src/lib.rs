// Copyright 2020 Oxide Computer Company

//! Simple proc macro consumer of `serde_tokenstream` that we use for testing
//! various failure cases.

use quote::quote;
use quote::ToTokens;
use serde::Deserialize;
use serde_tokenstream::from_tokenstream;
use serde_tokenstream::from_tokenstream_spanned;
use serde_tokenstream::ParseWrapper;
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
    bool_expr: Option<ParseWrapper<syn::Expr>>,
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
        Ok(attrs) => {
            let item = proc_macro2::TokenStream::from(item);

            let bool_assertion = attrs.bool_expr.map(|expr| {
                // Ensure that the bool_expr really is a boolean expression.
                let expr = expr.into_inner();
                quote! {
                    const _: bool = {
                        #expr
                    };
                }
            });

            quote! {
                #bool_assertion

                #item
            }
            .into()
        }

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
    let syn::Meta::List(list) = &annotation_attr.meta else {
        panic!("annotation attribute must be a list")
    };

    match from_tokenstream_spanned::<Annotation>(
        list.delimiter.span(),
        &list.tokens,
    ) {
        Ok(_) => {
            // Strip the annotation attribute from the function.
            let mut item = item.clone();
            item.attrs.retain(|attr| attr.path().is_ident("annotation"));
            item.into_token_stream().into()
        }
        Err(err) => err.to_compile_error().into(),
    }
}
