// Copyright 2026 Oxide Computer Company

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
    painted: Option<ParseWrapper<Painted>>,
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

/// An inline struct used to test span preservation in ParseWrapper.
#[derive(Deserialize)]
#[allow(dead_code)]
struct PaintColor {
    red: bool,
    green: bool,
    blue: bool,
}

/// A compound struct with a hand-written `Parse` impl that internally uses
/// `serde_tokenstream`. Used to test that `ParseWrapper` preserves span
/// information from the inner `syn::Error` rather than re-attributing it to the
/// surrounding group.
///
/// A `ParseWrapper` over a compound struct is not normally necessary -- our
/// case could just as well be `painted: Option<Painted>`. But it is necessary
/// when a hand-written `Parse` implementation must be provided, such as when
/// either a scalar value or a compound struct is allowed in a position.
#[allow(dead_code)]
struct Painted {
    color: PaintColor,
}

impl syn::parse::Parse for Painted {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        #[derive(Deserialize)]
        struct PaintedHelper {
            color: PaintColor,
        }

        let content;
        let brace_token = syn::braced!(content in input);
        let stream: proc_macro2::TokenStream = content.parse()?;
        let inner: PaintedHelper = serde_tokenstream::from_tokenstream_spanned(
            &brace_token.span,
            &stream,
        )?;
        Ok(Painted { color: inner.color })
    }
}

/// Used to test error attribution for `#[serde(flatten)]` fields, which serde
/// deserializes from buffered entries after the map has been consumed.
#[derive(Deserialize)]
#[allow(dead_code)]
struct Flattened {
    outer: u32,
    #[serde(flatten)]
    inner: FlattenedInner,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct FlattenedInner {
    needed: u32,
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
pub fn flattened(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<Flattened>(&attr.into()) {
        Ok(_) => item,
        Err(err) => err.to_compile_error().into(),
    }
}

// Like `outer`, but for a nested `flattened` attribute.
#[proc_macro_attribute]
pub fn flattened_outer(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = parse_macro_input!(item as syn::ItemFn);
    let flattened_attr =
        item.attrs.iter().find(|attr| attr.path().is_ident("flattened"));
    let flattened_attr = flattened_attr.expect("flattened attribute found");
    let syn::Meta::List(list) = &flattened_attr.meta else {
        panic!("flattened attribute must be a list")
    };

    match from_tokenstream_spanned::<Flattened>(
        list.delimiter.span(),
        &list.tokens,
    ) {
        Ok(_) => {
            let mut item = item.clone();
            item.attrs.retain(|attr| !attr.path().is_ident("flattened"));
            item.into_token_stream().into()
        }
        Err(err) => err.to_compile_error().into(),
    }
}

// Tests that a flatten error inside an array element is attributed to the
// element rather than the array.
#[derive(Deserialize)]
#[allow(dead_code)]
struct FlattenedList {
    items: Vec<Flattened>,
}

#[proc_macro_attribute]
pub fn flattened_list(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<FlattenedList>(&attr.into()) {
        Ok(_) => item,
        Err(err) => err.to_compile_error().into(),
    }
}

// Fixture to test deny_unknown_fields error attribution.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct Strict {
    a: u32,
}

#[proc_macro_attribute]
pub fn strict(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<Strict>(&attr.into()) {
        Ok(_) => item,
        Err(err) => err.to_compile_error().into(),
    }
}

// Fixture for a value whose Deserialize impl fails without reading a token.
#[derive(Deserialize)]
#[allow(dead_code)]
struct Rejected {
    #[serde(deserialize_with = "reject")]
    value: u32,
}

fn reject<'de, D: serde::Deserializer<'de>>(_: D) -> Result<u32, D::Error> {
    Err(serde::de::Error::custom("value is always rejected"))
}

#[proc_macro_attribute]
pub fn rejected(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<Rejected>(&attr.into()) {
        Ok(_) => item,
        Err(err) => err.to_compile_error().into(),
    }
}

// Fixture for TokenStreamWrapper values.
#[derive(Deserialize)]
#[allow(dead_code)]
struct Tokens {
    tokens: serde_tokenstream::TokenStreamWrapper,
}

#[proc_macro_attribute]
pub fn tokens(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<Tokens>(&attr.into()) {
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

// Used to test that top-level failures in situations where serde does internal
// buffering are reported properly (or at least as best as possible).
#[derive(Deserialize)]
#[serde(untagged)]
#[allow(dead_code)]
enum UntaggedConfig {
    A { a: u32 },
    B { b: u32 },
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
#[allow(dead_code)]
enum TaggedConfig {
    X { x: u32 },
    Y { y: u32 },
}

#[proc_macro_attribute]
pub fn untagged(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<UntaggedConfig>(&attr.into()) {
        Ok(_) => item,
        Err(err) => err.to_compile_error().into(),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct UntaggedList {
    items: Vec<UntaggedConfig>,
}

#[proc_macro_attribute]
pub fn untagged_list(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<UntaggedList>(&attr.into()) {
        Ok(_) => item,
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn tagged(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<TaggedConfig>(&attr.into()) {
        Ok(_) => item,
        Err(err) => err.to_compile_error().into(),
    }
}

// A fixture for try_from errors.
#[derive(Deserialize)]
#[serde(try_from = "EvenRaw")]
#[allow(dead_code)]
struct Even {
    n: u32,
}

#[derive(Deserialize)]
struct EvenRaw {
    n: u32,
}

impl std::convert::TryFrom<EvenRaw> for Even {
    type Error = String;

    fn try_from(raw: EvenRaw) -> Result<Self, Self::Error> {
        if raw.n % 2 == 0 {
            Ok(Even { n: raw.n })
        } else {
            Err(format!("{} is odd", raw.n))
        }
    }
}

#[proc_macro_attribute]
pub fn even(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<Even>(&attr.into()) {
        Ok(_) => item,
        Err(err) => err.to_compile_error().into(),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct EvenList {
    items: Vec<Even>,
}

#[proc_macro_attribute]
pub fn even_list(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<EvenList>(&attr.into()) {
        Ok(_) => item,
        Err(err) => err.to_compile_error().into(),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct NewtypeVariant {
    value: Wrapped,
}

#[derive(Deserialize)]
#[allow(dead_code)]
enum Wrapped {
    Named(String),
    Even(Even),
}

#[proc_macro_attribute]
pub fn newtype_variant(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match from_tokenstream::<NewtypeVariant>(&attr.into()) {
        Ok(_) => item,
        Err(err) => err.to_compile_error().into(),
    }
}
