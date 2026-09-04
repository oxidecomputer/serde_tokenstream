// Copyright 2026 Oxide Computer Company

use std::cell::RefCell;

use proc_macro2::{Span, TokenStream, TokenTree};
use serde::{Deserialize, de::Error, de::Visitor};
use syn::ext::IdentExt;

use crate::serde_tokenstream::spanned_error;

/// A wrapper around [`TokenStream`] that implements [`Deserialize`] in the
/// context of [`from_tokenstream`].
///
/// You can use this if, say, your macro allows users to pass in Rust tokens as
/// a configuration option. This can be useful, for example, in a macro that
/// generates code where the caller of that macro might want to augment the
/// generated code.
///
/// # Limitations
///
/// This type can only be deserialized within [`from_tokenstream`] or
/// [`from_tokenstream_spanned`], and only in positions where serde does not
/// perform internal buffering (e.g., it cannot be used inside
/// `#[serde(flatten)]` or `#[serde(untagged)]`). When used with internal
/// buffering, this produces an error.
///
/// # Panics
///
/// The [`Deserialize`] implementation for `TokenStreamWrapper` will panic if
/// it is not used in the context of [`from_tokenstream`] or
/// [`from_tokenstream_spanned`].
///
/// [`from_tokenstream`]: crate::from_tokenstream
/// [`from_tokenstream_spanned`]: crate::from_tokenstream_spanned
#[derive(Clone, Debug, Default)]
pub struct TokenStreamWrapper(TokenStream);

impl TokenStreamWrapper {
    pub fn into_inner(self) -> TokenStream {
        self.0
    }
}

impl From<TokenStream> for TokenStreamWrapper {
    fn from(inner: TokenStream) -> Self {
        Self(inner)
    }
}

impl<'de> Deserialize<'de> for TokenStreamWrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self(deserializer.deserialize_bytes(WrapperVisitor)?))
    }
}

impl std::ops::Deref for TokenStreamWrapper {
    type Target = TokenStream;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A wrapper around `syn`'s [`Parse`] trait that implements [`Deserialize`] in
/// the context of [`from_tokenstream`].
///
/// This extends [`TokenStreamWrapper`] by further interpreting the TokenStream
/// and guiding the user in the case of parse errors.
///
/// # Limitations
///
/// This type can only be deserialized within [`from_tokenstream`] or
/// [`from_tokenstream_spanned`], and only in positions where serde does not
/// perform internal buffering (e.g., it cannot be used inside
/// `#[serde(flatten)]` or `#[serde(untagged)]`). When used with internal
/// buffering, this produces an error.
///
/// # Panics
///
/// The [`Deserialize`] implementation for `ParseWrapper` will panic if it is
/// not used in the context of [`from_tokenstream`] or
/// [`from_tokenstream_spanned`].
///
/// [`Parse`]: syn::parse::Parse
/// [`from_tokenstream`]: crate::from_tokenstream
/// [`from_tokenstream_spanned`]: crate::from_tokenstream_spanned
#[derive(Clone, Debug, Default, Hash, Eq, PartialEq)]
pub struct ParseWrapper<P: syn::parse::Parse>(P);

impl<P: syn::parse::Parse> ParseWrapper<P> {
    pub fn into_inner(self) -> P {
        self.0
    }
}

impl<P: syn::parse::Parse> From<P> for ParseWrapper<P> {
    fn from(inner: P) -> Self {
        Self(inner)
    }
}

impl<'de, P: syn::parse::Parse> Deserialize<'de> for ParseWrapper<P> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let token_stream = deserializer.deserialize_bytes(WrapperVisitor)?;

        let parser = |input: syn::parse::ParseStream<'_>| -> syn::Result<P> {
            let parsed = P::parse(input)?;
            // The deserializer hands over every token up to the next `,`, `=`,
            // or EOF, so anything left after `P` is a stray token. Report such
            // tokens as errors.
            if let Some((tt, _)) = input.cursor().token_tree() {
                return Err(spanned_error(
                    &tt,
                    format!("expected `,` or nothing, but found `{tt}`"),
                ));
            }
            Ok(parsed)
        };

        match syn::parse::Parser::parse2(parser, token_stream) {
            Ok(parsed) => Ok(Self(parsed)),
            Err(err) => {
                let msg = err.to_string();
                set_parse_error(err);
                Err(D::Error::custom(msg))
            }
        }
    }
}

impl<P: syn::parse::Parse> std::ops::Deref for ParseWrapper<P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A string value that remembers its span.
///
/// Use this as [`ParseWrapper`]`<SpannedString>`. It accepts the same inputs
/// as [`String`] (a string literal or a bare identifier), but also records the
/// span of that token. Macros that interpret a string further (as an
/// identifier, a file name, and so on) can use the span to report errors at
/// the value itself rather than at the attribute as a whole.
///
/// The [`parse`](Self::parse) and [`parse_with`](Self::parse_with) methods
/// parse the value as Rust syntax, similar to [`syn::LitStr::parse`]. The
/// resulting types and errors have the correct span information associated with
/// them.
///
/// Equality and hashing compare only the value, not the span.
///
/// # Example
///
/// ```
/// use quote::quote;
/// use serde::Deserialize;
/// use serde_tokenstream::{ParseWrapper, SpannedString, from_tokenstream};
///
/// #[derive(Deserialize)]
/// struct Config {
///     module: ParseWrapper<SpannedString>,
/// }
///
/// // In a proc macro, this would be the macro's input.
/// let attr = quote! { module = "not a module" };
/// let config = from_tokenstream::<Config>(&attr)?;
///
/// // Interpret the value further, reporting errors at the value rather than
/// // at the attribute as a whole.
/// let module = config.module.parse::<syn::Ident>().map_err(|err| {
///     syn::Error::new(
///         config.module.span(),
///         format!(
///             "`{}` is not a valid module name: {err}",
///             config.module.value()
///         ),
///     )
/// });
/// assert!(module.is_err());
/// # Ok::<(), syn::Error>(())
/// ```
///
/// # Limitations
///
/// See the [`ParseWrapper`] documentation for limitations.
///
/// [`from_tokenstream`]: crate::from_tokenstream
/// [`from_tokenstream_spanned`]: crate::from_tokenstream_spanned
#[derive(Debug, Clone)]
pub struct SpannedString {
    value: String,
    span: Span,
}

impl SpannedString {
    /// Creates a `SpannedString` from a value and a span.
    ///
    /// Deserializing a `ParseWrapper<SpannedString>` is the usual way to
    /// obtain a `SpannedString` -- this is for cases like default values and
    /// tests.
    pub fn new(value: impl Into<String>, span: Span) -> Self {
        Self { value: value.into(), span }
    }

    /// Returns the value.
    ///
    /// In case of a string literal:
    ///
    /// - The quotes are stripped from the value.
    /// - Escapes are processed, so that (e.g.) `"\n"` becomes a newline.
    ///
    /// Identifiers are stored verbatim, so raw identifiers keep the `r#` prefix.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns the span of the token the value was written as.
    pub fn span(&self) -> Span {
        self.span
    }

    /// Returns the value, discarding the span.
    pub fn into_string(self) -> String {
        self.value
    }

    /// Parses the value as a `T`.
    ///
    /// In both success and error cases, the span points to this value.
    pub fn parse<T: syn::parse::Parse>(&self) -> syn::Result<T> {
        self.parse_with(T::parse)
    }

    /// Invokes `parser` on the value.
    ///
    /// In both success and error cases, the span points to this value.
    pub fn parse_with<F: syn::parse::Parser>(
        &self,
        parser: F,
    ) -> syn::Result<F::Output> {
        syn::LitStr::new(&self.value, self.span).parse_with(parser)
    }
}

impl AsRef<str> for SpannedString {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Display for SpannedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.value)
    }
}

impl PartialEq for SpannedString {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for SpannedString {}

impl std::hash::Hash for SpannedString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl syn::parse::Parse for SpannedString {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let value = if input.peek(syn::LitStr) {
            let lit: syn::LitStr = input.parse()?;
            Self { value: lit.value(), span: lit.span() }
        } else if input.peek(syn::Ident::peek_any) {
            // Keywords are accepted, as they are for `String`.
            let ident = syn::Ident::parse_any(input)?;
            Self { value: ident.to_string(), span: ident.span() }
        } else {
            return Err(match input.cursor().token_tree() {
                Some((tt, _)) => spanned_error(
                    &tt,
                    format!("expected a string, but found `{tt}`"),
                ),
                None => input.error("expected a string"),
            });
        };

        Ok(value)
    }
}

/// We would like to be able to pass `TokenStream`s through unperturbed, but
/// that isn't directly possible with serde's model, because
/// serde--wisely--does not permit this kind of unholy communion between
/// Deserialize and Deserializer.
///
/// However, we can skirt around this with the otherwise-unused
/// deserialize_bytes/visit_bytes interfaces. Since there is no `TokenStream`
/// that could reasonably be interpreted as bytes, we use this interface to
/// signal to the `serde_tokenstream` deserializer that we should be
/// interpreting the TokenStream directly.
///
/// The mechanism works via a thread-local storage (TLS) side channel. When we
/// want to interpret a TokenStream directly:
///
/// 1. First, `TokenStreamWrapper` or `ParseWrapper` calls
///    `deserializer.deserialize_bytes(WrapperVisitor)`, assuming that
///    `deserializer` is always the `serde_tokenstream` deserializer.
/// 2. The `serde_tokenstream` deserializer calls `set_wrapper_tokens` with the
///    `TokenStream`.
/// 3. The `serde_tokenstream` deserializer calls `visitor.visit_bytes`, which
///    is always the `WrapperVisitor`.
/// 4. The `WrapperVisitor` deserializer immediately calls
///    `take_wrapper_tokens` to retrieve the `TokenStream`.
///
/// So, yes: this is ick. However, unlike some alternatives like serializing
/// TokenStreams to bytes, this approach allows us to retain Span information.
/// In turn, that allows us to craft very good, targeted errors to guide users
/// in the case of bad input.
struct WrapperVisitor;

impl Visitor<'_> for WrapperVisitor {
    type Value = TokenStream;

    fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        // Serde shows this text to macro users in case of a wrapper being used
        // with internal buffering. (Not for untagged, though, unfortunately,
        // because serde swallows errors from untagged variants. Why does
        // untagged exist at all if the UX is so bad? Great question, and the
        // answer will be a mystery.)
        formatter.write_str(
            "a ParseWrapper or TokenStreamWrapper value; these cannot be used \
             inside `#[serde(flatten)]`, `#[serde(untagged)]`, or similar -- \
             this is a bug in the macro",
        )
    }

    fn visit_bytes<E>(self, bytes: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        assert!(
            bytes.is_empty(),
            "visit_bytes should always be called with an empty slice \
             (a side channel is used to pass the actual TokenStream;
             was TokenStreamWrapper or ParseWrapper used outside of a
             serde_tokenstream context?)"
        );
        Ok(take_wrapper_tokens())
    }
}

thread_local! {
    // This acts as a side channel to pass information around between
    // visit_bytes and deserialize_bytes. It's fine...
    //
    // Instead of a `Vec<TokenStream>` representing a stack, it's okay to use a
    // single Option here because we read data back from it immediately after
    // writing to it. The order of operations is as follows:
    //
    // 1. `deserialize_bytes` in `serde_tokenstream.rs` calls
    //    `set_wrapper_tokens`.
    // 2. `deserialize_bytes` calls `WrapperVisitor::visit_bytes` immediately
    //    afterwards.
    // 3. `visit_bytes` calls `take_wrapper_tokens` to retrieve the tokens.
    // 4. Only after that does any potential syn parsing of the token stream
    //    occur.
    //
    // Because this set/take sequence is immediate without anything in between,
    // there's no nesting to be worried about.
    static WRAPPER_TOKENS: RefCell<Option<TokenStream>> = Default::default();

    // A second side channel for preserving span information from syn parse
    // errors through serde's `D::Error::custom` bottleneck.
    //
    // When `ParseWrapper::deserialize` calls `syn::parse2` and it fails, the
    // resulting `syn::Error` carries precise span information. But
    // `D::Error::custom` only accepts `T: Display`, flattening the error to a
    // string and losing that span. To preserve it:
    //
    // 1. `ParseWrapper::deserialize` stores the `syn::Error` here via
    //    `set_parse_error`.
    // 2. `ParseWrapper::deserialize` calls `D::Error::custom(msg)`.
    // 3. `InternalError::custom` calls `take_parse_error` and, if set,
    //    returns `InternalError::Spanned(syn_error)` instead of
    //    `InternalError::Unspanned(msg)`.
    //
    // As with `WRAPPER_TOKENS`, the set/take sequence is immediate.
    static PARSE_ERROR: RefCell<Option<syn::Error>> = Default::default();
}

pub(crate) fn set_wrapper_tokens(tokens: Vec<TokenTree>) {
    WRAPPER_TOKENS.with(|cell| {
        let mut cell = cell.borrow_mut();
        assert!(cell.is_none(), "set_wrapper_tokens requires TLS to be unset");
        *cell = Some(tokens.into_iter().collect());
    });
}

fn take_wrapper_tokens() -> TokenStream {
    WRAPPER_TOKENS.with(|cell| {
        cell.borrow_mut().take().expect(
            "take_wrapper_tokens requires TLS to be set \
             (was TokenStreamWrapper or ParseWrapper used
             outside of a serde_tokenstream context?)",
        )
    })
}

fn set_parse_error(err: syn::Error) {
    PARSE_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(err);
    });
}

pub(crate) fn take_parse_error() -> Option<syn::Error> {
    PARSE_ERROR.with(|cell| cell.borrow_mut().take())
}
