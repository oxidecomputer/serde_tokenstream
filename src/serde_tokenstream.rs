// Copyright 2026 Oxide Computer Company

use core::iter::Peekable;
use std::{
    any::type_name,
    fmt::{self, Display},
};

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree, extra::DelimSpan};
use quote::ToTokens;
use serde::de::{
    DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor,
};
use serde::{Deserialize, Deserializer};
use syn::{ExprLit, Lit};

use crate::ibidem::set_wrapper_tokens;
use crate::ibidem::take_parse_error;

/// Alias for `syn::Error`.
///
/// `syn::Error` already does the heavy lifting of massaging errors for
/// consumption by the compiler so we lean on that report deserialization
/// errors so that the compiler reports and renders them appropriately.
pub type Error = syn::Error;

/// Alias for a Result with the error type serde_tokenstream::Error.
pub type Result<T> = std::result::Result<T, Error>;

/// Deserialize an instance of type `T` from a [`TokenStream`].
///
/// # Example
/// ```
/// use quote::quote;
/// use serde::Deserialize;
/// use serde_tokenstream::from_tokenstream;
/// use serde_tokenstream::Result;
///
/// fn main() -> Result<()> {
///     #[derive(Deserialize)]
///     struct Record {
///         worker: String,
///         floor: u32,
///         region: String,
///     }
///     let tokenstream = quote! {
///         worker = "Homer J. Simpson",
///         floor = 7,
///         region = "G",
///     };
///
///     let rec = from_tokenstream::<Record>(&tokenstream)?;
///     println!("{} {}{}", rec.worker, rec.floor, rec.region);
///     Ok(())
/// }
/// ```
pub fn from_tokenstream<'a, T>(tokens: &'a TokenStream) -> Result<T>
where
    T: Deserialize<'a>,
{
    from_tokenstream_impl(None, tokens)
}

/// Deserialize an instance of type `T` from a [`TokenStream`] with data
/// inside, along with a [`DelimSpan`] for the surrounding braces.
///
/// This is useful when parsing an attribute nested inside an outer macro. In
/// that case, better span information (not just `Span::call_site`) can be
/// produced.
///
/// # Example
///
/// The most common use is with [`syn::MetaList`] instances. For example, if
/// your macro is `#[derive(Record)]` and you're invoked like this:
///
/// ```rust,ignore
/// #[derive(Record)]
/// #[record { worker = "Homer J. Simpson", floor = 7, region = "G" }]
/// fn test() {}
/// ```
///
/// Then, the `record` attribute inside can be interpreted as a
/// `syn::MetaList`. With it in hand:
///
/// ```
/// use syn::parse_quote;
/// use serde::Deserialize;
/// use serde_tokenstream::from_tokenstream_spanned;
/// use serde_tokenstream::Result;
///
/// fn main() -> Result<()> {
///     #[derive(Deserialize)]
///     struct Record {
///         worker: String,
///         floor: u32,
///         region: String,
///     }
///
///     // This is the `syn::MetaList` instance above.
///     let list: syn::MetaList = parse_quote! {
///         record {
///             worker = "Homer J. Simpson",
///             floor = 7,
///             region = "G",
///         }
///     };
///
///     let rec = from_tokenstream_spanned::<Record>(list.delimiter.span(), &list.tokens)?;
///     println!("{} {}{}", rec.worker, rec.floor, rec.region);
///     Ok(())
/// }
/// ```
///
/// If there's an error like a missing field, it will now be reported with the
/// span of the braces inside the `record` attribute (whereas
/// [`from_tokenstream`] lacks the necessary [`Span`] information).
///
/// [`Span`]: proc_macro2::Span
pub fn from_tokenstream_spanned<'a, T>(
    span: &DelimSpan,
    tokens: &'a TokenStream,
) -> Result<T>
where
    T: Deserialize<'a>,
{
    from_tokenstream_impl(Some(span), tokens)
}

fn from_tokenstream_impl<'a, T>(
    span: Option<&DelimSpan>,
    input: &'a TokenStream,
) -> Result<T>
where
    T: Deserialize<'a>,
{
    // We implicitly start inside a brace-surrounded struct.
    // Constructing a Group allows for more generic handling.
    // If there is an error at the top level (such as a missing field) it
    // will be attributed to the span of the group, which is why we let
    // users optionally include the span for attribution.
    let mut group = Group::new(Delimiter::Brace, input.clone());
    if let Some(span) = span {
        group.set_span(span.join());
    }
    let mut deserializer = TokenDe::new(
        &group,
        &TokenStream::from(TokenTree::from(group.clone())),
    );

    let result = T::deserialize(&mut deserializer)
        .map_err(|err| err.into_error(&group))?;

    // On success, check that there aren't additional, unparsed tokens.
    match deserializer.next() {
        None => Ok(result),
        Some(token) => Err(spanned_error(
            &token,
            format!("expected EOF but found `{}`", token),
        )),
    }
}

#[derive(Clone, Debug)]
enum InternalError {
    Spanned(Error),
    Unspanned(String),
}

impl InternalError {
    /// Converts self to a `syn::Error`, assigning the `token` span if one isn't
    /// already available.
    fn into_error(self, fallback: impl ToTokens) -> Error {
        match self {
            InternalError::Spanned(err) => err,
            InternalError::Unspanned(msg) => spanned_error(fallback, msg),
        }
    }

    /// Converts `Self::Unspanned` to `Self::Spanned` with the `token` span.
    fn or_at(self, fallback: impl ToTokens) -> Self {
        InternalError::Spanned(self.into_error(fallback))
    }
}

/// A better `syn::Error::new_spanned`.
///
/// With `macro_rules` macros, `syn::Error::new_spanned` on a proc macro
/// invocation inside the declarative macro would result in the span being the
/// macro declaration. This constructor pierces through that to ensure better
/// diagnostic reporting for this cases.
///
/// # Example
///
/// Consider a declarative macro that forwards an expression into an attribute
/// macro that uses `serde_tokenstream`:
///
/// ```text
/// macro_rules! wrap {
///     ($s:expr) => {
///         #[annotation {
///             string = $s,
///             options = OptionA,
///             unit = (),
///             tup = (1, 2.0),
///         }]
///         fn test() {}
///     };
/// }
///
/// wrap!(foo::bar);
/// ```
///
/// `string` is expected to be an identifier, but a path `foo::bar` is passed
/// in. In this case, `syn::Error::new_spanned` takes the spans of the first and
/// last tokens it is given, which are both the group, so the diagnostic points to
/// the definition:
///
/// ```text
/// error: expected a string, but found `foo::bar`
///   --> tests/ui/bad_string_from_macro_rules_path.rs:11:22
///    |
/// 11 |             string = $s,
///    |                      ^^
/// ...
/// 20 | wrap!(foo::bar);
///    | --------------- in this macro invocation
/// ```
///
/// What `spanned_error` does is replace `Delimiter::None` groups with their contents
/// (recursively, to handle nested declarative macros) before taking the first
/// and last spans, so the same error is reported at the invocation:
///
/// ```text
/// error: expected a string, but found `foo::bar`
///   --> tests/ui/bad_string_from_macro_rules_path.rs:20:7
///    |
/// 20 | wrap!(foo::bar);
///    |       ^^^^^^^^
/// ```
///
/// An empty substitution (for example an empty `$v:vis`) has nothing to
/// underline, so the group's own span is used as a fallback.
#[expect(clippy::disallowed_methods)]
pub fn spanned_error(tokens: impl ToTokens, msg: impl Display) -> Error {
    let tokens = tokens.into_token_stream();
    let substituted = flatten_none_groups(tokens.clone());
    if substituted.is_empty() {
        // If flattening groups resulted in nothing, we've got to have
        // _something_ to point to. Use tokens as a fallback.
        Error::new_spanned(tokens, msg)
    } else {
        Error::new_spanned(substituted, msg)
    }
}

/// Replaces `Delimiter::None` groups with their contents, recursively.
///
/// `Delimiter::None` groups can occur with declarative macro substitutions.
fn flatten_none_groups(stream: TokenStream) -> TokenStream {
    stream
        .into_iter()
        .flat_map(|tt| match tt {
            TokenTree::Group(group) if group.delimiter() == Delimiter::None => {
                flatten_none_groups(group.stream())
            }
            TokenTree::Group(_)
            | TokenTree::Ident(_)
            | TokenTree::Punct(_)
            | TokenTree::Literal(_) => TokenStream::from(tt),
        })
        .collect()
}

/// Returns true if this token tree is an empty `macro_rules!` substitution.
fn is_empty_substitution(tt: &TokenTree) -> bool {
    match tt {
        TokenTree::Group(group) if group.delimiter() == Delimiter::None => {
            // Recursively check to deal with nested macro invocations.
            group.stream().into_iter().all(|tt| is_empty_substitution(&tt))
        }
        TokenTree::Group(_)
        | TokenTree::Ident(_)
        | TokenTree::Punct(_)
        | TokenTree::Literal(_) => false,
    }
}

type InternalResult<T> = std::result::Result<T, InternalError>;

struct TokenDe {
    input: Peekable<Box<dyn Iterator<Item = TokenTree>>>,
    current: Option<TokenTree>,
    last: Option<TokenTree>,
    pending_member: bool,
    // The group whose contents this deserializer is currently reading. This is
    // used as a fallback in case a more specific span isn't available.
    enclosing: Group,
}

impl TokenDe {
    fn new(enclosing: &Group, input: &TokenStream) -> Self {
        let t: Box<dyn Iterator<Item = TokenTree>> =
            Box::new(input.clone().into_iter());
        TokenDe {
            input: t.peekable(),
            current: None,
            last: None,
            pending_member: false,
            enclosing: enclosing.clone(),
        }
    }

    fn gobble_optional_comma(&mut self) -> InternalResult<()> {
        match self.next() {
            None => Ok(()),
            Some(TokenTree::Punct(punct)) if punct.as_char() == ',' => Ok(()),
            Some(token) => Err(InternalError::Spanned(spanned_error(
                &token,
                format!("expected `,` or nothing, but found `{}`", token),
            ))),
        }
    }

    // Skip over empty macro_rules substitutions.
    //
    // Taking care of them here means that other parts of the code don't need to
    // worry about them -- so `next` and `peek` never return one, and
    // `last` and `current` never point at one.
    fn skip_empty_substitutions(&mut self) {
        while self.input.peek().is_some_and(is_empty_substitution) {
            self.input.next();
        }
    }

    fn peek(&mut self) -> Option<&TokenTree> {
        self.skip_empty_substitutions();
        self.input.peek()
    }

    fn next(&mut self) -> Option<TokenTree> {
        self.skip_empty_substitutions();
        let next = self.input.next();

        self.last =
            std::mem::replace(&mut self.current, next.as_ref().cloned());
        next
    }

    fn previous(&self) -> Option<TokenTree> {
        self.current.clone().or_else(|| self.last.clone())
    }

    fn last_err<T>(&self, what: &str) -> InternalResult<T> {
        match &self.last {
            Some(token) => Err(InternalError::Spanned(spanned_error(
                token,
                format!("expected {} following `{}`", what, token),
            ))),
            // Nothing's been read yet, so the enclosing group is empty. This
            // can happen in situations like `V()`.
            None => Err(InternalError::Spanned(spanned_error(
                &self.enclosing,
                format!("expected {} inside `{}`", what, self.enclosing),
            ))),
        }
    }

    // Rejects tokens left over inside a parenthesized group after a fixed
    // number of values has been read.
    fn expect_close_paren(&mut self) -> InternalResult<()> {
        match self.next() {
            None => Ok(()),
            Some(token) => Err(InternalError::Spanned(spanned_error(
                &token,
                format!("expected `)`, but found `{}`", token),
            ))),
        }
    }

    fn deserialize_error<VV>(
        &self,
        next: Option<TokenTree>,
        what: &str,
    ) -> InternalResult<VV> {
        match next {
            Some(token) => Err(InternalError::Spanned(spanned_error(
                &token,
                format!("expected {}, but found `{}`", what, token),
            ))),
            None => self.last_err(what),
        }
    }

    /// Consumes the next token if it is a Delimiter::None group.
    ///
    /// These groups occur with declarative macro substitutions.
    ///
    /// Every `deserialize_*` method that reads a token (other than
    /// `deserialize_bytes` which is special) starts by checking this.
    fn take_transparent(&mut self) -> Option<Group> {
        let group = match self.peek() {
            Some(TokenTree::Group(group))
                if group.delimiter() == Delimiter::None =>
            {
                group.clone()
            }
            Some(TokenTree::Group(_))
            | Some(TokenTree::Ident(_))
            | Some(TokenTree::Punct(_))
            | Some(TokenTree::Literal(_))
            | None => return None,
        };
        self.next().map(|_| group)
    }

    /// Deserializes the value obtained from `take_transparent`.
    fn deserialize_transparent<V, F>(
        group: &Group,
        what: &str,
        deserialize: F,
    ) -> InternalResult<V>
    where
        F: FnOnce(&mut TokenDe) -> InternalResult<V>,
    {
        // Flatten nested groups from recursive macro_rules.
        let stream = flatten_none_groups(group.stream());

        let mut inner = TokenDe::new(group, &stream);
        let value = deserialize(&mut inner)?;

        match inner.next() {
            None => Ok(value),
            Some(_) => Err(InternalError::Spanned(spanned_error(
                group,
                format!("expected only {}, but found `{}`", what, group),
            ))),
        }
    }

    fn deserialize_int<T, VV, F>(&mut self, visit: F) -> InternalResult<VV>
    where
        F: FnOnce(T) -> InternalResult<VV>,
        T: std::str::FromStr,
        T::Err: Display,
    {
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(
                &group,
                type_name::<T>(),
                |inner| inner.deserialize_int(visit),
            );
        }

        let next = self.next();

        let mut stream = Vec::new();

        let next_next = match &next {
            Some(tt @ TokenTree::Punct(p)) if p.as_char() == '-' => {
                stream.push(tt.clone());
                self.next()
            }
            any => any.clone(),
        };

        if let Some(tt) = next_next {
            stream.push(tt);

            if let Ok(i) =
                syn::parse2::<syn::LitInt>(stream.into_iter().collect())
            {
                if let Ok(value) = i.base10_parse::<T>() {
                    return visit(value);
                }
            }
        }

        self.deserialize_error(next, type_name::<T>())
    }

    fn deserialize_float<T, VV, F>(&mut self, visit: F) -> InternalResult<VV>
    where
        F: FnOnce(T) -> InternalResult<VV>,
        T: std::str::FromStr,
        T::Err: Display,
    {
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(
                &group,
                type_name::<T>(),
                |inner| inner.deserialize_float(visit),
            );
        }

        let next = self.next();

        let mut stream = Vec::new();

        let next_next = match &next {
            Some(tt @ TokenTree::Punct(p)) if p.as_char() == '-' => {
                stream.push(tt.clone());
                self.next()
            }
            any => any.clone(),
        };

        if let Some(tt) = next_next {
            stream.push(tt);

            let parsed =
                match syn::parse2::<ExprLit>(stream.into_iter().collect()) {
                    Ok(ExprLit { lit: Lit::Int(i), .. }) => {
                        i.base10_parse::<T>().ok()
                    }
                    Ok(ExprLit { lit: Lit::Float(f), .. }) => {
                        f.base10_parse::<T>().ok()
                    }
                    _ => None,
                };

            if let Some(value) = parsed {
                return visit(value);
            }
        }

        self.deserialize_error(next, type_name::<T>())
    }
}

impl serde::de::Error for InternalError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        // Check whether a ParseWrapper stored a syn::Error with span
        // information via the PARSE_ERROR side channel. If so, use it
        // directly to preserve the span.
        if let Some(parse_err) = take_parse_error() {
            InternalError::Spanned(parse_err)
        } else {
            InternalError::Unspanned(format!("{}", msg))
        }
    }
}
impl std::error::Error for InternalError {}

impl Display for InternalError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(format!("{:?}", self).as_str())
    }
}

impl<'de> MapAccess<'de> for TokenDe {
    type Error = InternalError;

    fn next_key_seed<K>(&mut self, seed: K) -> InternalResult<Option<K::Value>>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        let keytok = match self.peek() {
            None => return Ok(None),
            Some(token) => token.clone(),
        };

        let key = seed
            .deserialize(&mut *self)
            .map(Some)
            .map_err(|err| err.or_at(&keytok));

        // Verify we have an '=' delimiter.
        if key.is_ok() {
            let _eq = match self.next() {
                Some(TokenTree::Punct(punct)) if punct.as_char() == '=' => {
                    punct
                }

                Some(token) => {
                    return Err(InternalError::Spanned(spanned_error(
                        &token,
                        format!("expected `=`, but found `{}`", token),
                    )));
                }
                None => {
                    return Err(InternalError::Spanned(spanned_error(
                        &keytok,
                        format!("expected `=` following `{}`", keytok),
                    )));
                }
            };
        }

        // We expect to have an object member.
        self.pending_member = true;

        key
    }

    fn next_value_seed<V>(&mut self, seed: V) -> InternalResult<V::Value>
    where
        V: DeserializeSeed<'de>,
    {
        let valtok = self.peek().cloned();
        let value = seed.deserialize(&mut *self);

        // We've processed the expected member via seed.deserialize.
        self.pending_member = false;

        // If a value token is not present, fall back to the current token (the
        // `=`) or the enclosing group.
        let value = value.map_err(|err| match (&valtok, &self.current) {
            (Some(token), _) | (None, Some(token)) => err.or_at(token),
            (None, None) => err.or_at(&self.enclosing),
        });

        if value.is_ok() {
            self.gobble_optional_comma()?;
        }

        value
    }
}

impl<'de> SeqAccess<'de> for TokenDe {
    type Error = InternalError;

    fn next_element_seed<T>(
        &mut self,
        seed: T,
    ) -> InternalResult<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        let eltok = match self.peek() {
            None => return Ok(None),
            Some(token) => token.clone(),
        };
        let value = seed
            .deserialize(&mut *self)
            .map(Some)
            .map_err(|err| err.or_at(&eltok));
        if value.is_ok() {
            self.gobble_optional_comma()?;
        }
        value
    }
}

impl<'de> EnumAccess<'de> for &mut TokenDe {
    type Error = InternalError;
    type Variant = Self;

    fn variant_seed<V>(
        self,
        seed: V,
    ) -> InternalResult<(V::Value, Self::Variant)>
    where
        V: DeserializeSeed<'de>,
    {
        let val = seed.deserialize(&mut *self);

        // If there was an error from serde, tag it with the current token, or
        // the enclosing token if not available.
        val.map_err(|err| match &self.current {
            Some(token) => err.or_at(token),
            None => err.or_at(&self.enclosing),
        })
        .map(|v| (v, self))
    }
}

impl<'de> VariantAccess<'de> for &mut TokenDe {
    type Error = InternalError;

    fn unit_variant(self) -> InternalResult<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> InternalResult<T::Value>
    where
        T: DeserializeSeed<'de>,
    {
        let next = self.next();

        if let Some(TokenTree::Group(group)) = &next {
            if let Delimiter::Parenthesis = group.delimiter() {
                let mut inner = TokenDe::new(group, &group.stream());
                // Attribute errors to the first token inside the parentheses,
                // similar to next_value_seed. Fall back to the group if it is
                // empty.
                let valtok = inner.input.peek().cloned();
                let value =
                    seed.deserialize(&mut inner).map_err(
                        |err| match &valtok {
                            Some(token) => err.or_at(token),
                            None => err.or_at(group),
                        },
                    )?;
                inner.expect_close_paren()?;
                return Ok(value);
            }
        }
        self.deserialize_error(next, "(")
    }

    fn tuple_variant<V>(
        self,
        _len: usize,
        visitor: V,
    ) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        let next = self.next();

        if let Some(token) = &next {
            if let TokenTree::Group(group) = token {
                if let Delimiter::Parenthesis = group.delimiter() {
                    let mut inner = TokenDe::new(group, &group.stream());
                    let value = visitor
                        .visit_seq(&mut inner)
                        .map_err(|err| err.or_at(token))?;
                    inner.expect_close_paren()?;
                    return Ok(value);
                }
            }
        }

        self.deserialize_error(next, "(")
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        let next = self.next();

        if let Some(token) = &next {
            if let TokenTree::Group(group) = token {
                if let Delimiter::Brace = group.delimiter() {
                    // TODO we should pass fields through to the new TokenDe and
                    // then use that rather than the call to
                    // deserialize_ignored_any to determine if the
                    // given field is valid.
                    return visitor
                        .visit_map(TokenDe::new(group, &group.stream()))
                        .map_err(|err| err.or_at(token));
                }
            }
        };

        self.deserialize_error(next, "{")
    }
}

/// Stub out Deserializer trait functions we don't want to deal with right now.
macro_rules! de_unimp {
    ($i:ident $(, $p:ident : $t:ty )*) => {
        fn $i<V>(self $(, $p: $t)*, _visitor: V) -> InternalResult<V::Value>
        where
            V: Visitor<'de>,
        {
            unimplemented!(stringify!($i));
        }
    };
    ($i:ident $(, $p:ident : $t:ty )* ,) => {
        de_unimp!($i $(, $p: $t)*);
    };
}

impl<'de> Deserializer<'de> for &mut TokenDe {
    type Error = InternalError;

    fn deserialize_bool<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(&group, "bool", |inner| {
                inner.deserialize_bool(visitor)
            });
        }

        match self.next() {
            Some(TokenTree::Ident(ident)) if ident == "true" => {
                visitor.visit_bool(true)
            }
            Some(TokenTree::Ident(ident)) if ident == "false" => {
                visitor.visit_bool(false)
            }
            other => self.deserialize_error(other, "bool"),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        // None is a missing field, so this must be Some.
        visitor.visit_some(self)
    }

    fn deserialize_string<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(
                &group,
                "a string",
                |inner| inner.deserialize_string(visitor),
            );
        }

        let token = self.next();
        let value = match &token {
            Some(TokenTree::Ident(ident)) => Some(ident.to_string()),
            Some(tt) => {
                match syn::parse2::<syn::LitStr>(TokenStream::from(tt.clone()))
                {
                    Ok(s) => Some(s.value()),
                    _ => None,
                }
            }
            _ => None,
        };

        value.map_or_else(
            || self.deserialize_error(token, "a string"),
            |v| visitor.visit_string(v),
        )
    }
    fn deserialize_str<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(
                &group,
                "an array",
                |inner| inner.deserialize_seq(visitor),
            );
        }

        let next = self.next();

        if let Some(TokenTree::Group(group)) = &next {
            if let Delimiter::Bracket = group.delimiter() {
                return visitor.visit_seq(TokenDe::new(group, &group.stream()));
            }
        }

        self.deserialize_error(next, "an array")
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(
                &group,
                "a struct",
                |inner| inner.deserialize_struct(name, fields, visitor),
            );
        }

        let next = self.next();

        if let Some(TokenTree::Group(group)) = &next {
            if let Delimiter::Brace = group.delimiter() {
                // TODO we should pass fields through to the new TokenDe and
                // then use that rather than the call to
                // deserialize_ignored_any to determine if the
                // given field is valid.
                return visitor.visit_map(TokenDe::new(group, &group.stream()));
            }
        };

        self.deserialize_error(next, "a struct")
    }

    fn deserialize_map<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(
                &group,
                "a map",
                |inner| inner.deserialize_map(visitor),
            );
        }

        let next = self.next();

        if let Some(TokenTree::Group(group)) = &next {
            if let Delimiter::Brace = group.delimiter() {
                return visitor.visit_map(TokenDe::new(group, &group.stream()));
            }
        }

        self.deserialize_error(next, "a map")
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        // Do the transparent business here rather than in `deserialize_identifier`.
        //
        // For example, with an enum `Kind { A, B(u32) }` and a declarative
        // macro that forwards an expression into the attribute:
        //
        //     macro_rules! wrap {
        //         ($k:expr) => {
        //             #[annotation { kind = $k }]
        //             fn test() {}
        //         };
        //     }
        //
        //     wrap!(B(4));
        //
        // the value corresponding to `kind` is a single `Delimiter::None` group
        // containing two token trees, `B` and `(4)`:
        //
        //     kind = ⟨B (4)⟩
        //
        // When deserializing an enum, we would read the variant name via
        // `deserialize_identifier`, and then the payload via
        // `newtype_variant_seed`. Unwrapping the group here lets both reads
        // happen over here, so the trailing-token check sees nothing left over.
        // If the group were unwrapped in `deserialize_identifier` instead, only
        // `B` would be read from inside it and `(4)` would be rejected as a
        // stray token.
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(
                &group,
                &format!("a variant of `{}`", name),
                |inner| inner.deserialize_enum(name, variants, visitor),
            );
        }

        visitor.visit_enum(self)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(
                &group,
                "an identifier",
                |inner| inner.deserialize_identifier(visitor),
            );
        }

        let next = self.next();

        if let Some(ident @ TokenTree::Ident(_)) = next {
            return visitor.visit_string(ident.to_string());
        }

        self.deserialize_error(next, "an identifier")
    }

    fn deserialize_char<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(
                &group,
                "a char",
                |inner| inner.deserialize_char(visitor),
            );
        }

        let next = self.next();

        if let Some(tt) = &next {
            if let Ok(ch) =
                syn::parse2::<syn::LitChar>(TokenStream::from(tt.clone()))
            {
                return visitor.visit_char(ch.value());
            }
        }

        self.deserialize_error(next, "a char")
    }

    fn deserialize_unit<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(
                &group,
                "a unit",
                |inner| inner.deserialize_unit(visitor),
            );
        }

        let next = self.next();

        if let Some(TokenTree::Group(group)) = &next {
            if let Delimiter::Parenthesis = group.delimiter() {
                if group.stream().is_empty() {
                    return visitor.visit_unit();
                }
            }
        }

        self.deserialize_error(next, "a unit")
    }

    fn deserialize_tuple<V>(
        self,
        len: usize,
        visitor: V,
    ) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        if let Some(group) = self.take_transparent() {
            return TokenDe::deserialize_transparent(
                &group,
                "a tuple",
                |inner| inner.deserialize_tuple(len, visitor),
            );
        }

        let next = self.next();

        if let Some(TokenTree::Group(group)) = &next {
            if let Delimiter::Parenthesis = group.delimiter() {
                let mut inner = TokenDe::new(group, &group.stream());
                let value = visitor.visit_seq(&mut inner)?;
                inner.expect_close_paren()?;
                return Ok(value);
            }
        }

        self.deserialize_error(next, "a tuple")
    }

    fn deserialize_any<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        let token = self.next();

        match &token {
            None => self.last_err("a value"),
            Some(TokenTree::Group(group)) => match group.delimiter() {
                Delimiter::Brace => {
                    visitor.visit_map(TokenDe::new(group, &group.stream()))
                }
                Delimiter::Bracket => {
                    visitor.visit_seq(TokenDe::new(group, &group.stream()))
                }
                Delimiter::Parenthesis => {
                    let stream = &group.stream();
                    if stream.is_empty() {
                        visitor.visit_unit()
                    } else {
                        visitor.visit_seq(TokenDe::new(group, stream))
                    }
                }
                // A None delimiter occurs for a macro_rules! substitution. We
                // can simply descend into those tokens.
                Delimiter::None => TokenDe::deserialize_transparent(
                    group,
                    "a value",
                    |inner| inner.deserialize_any(visitor),
                ),
            },
            Some(TokenTree::Ident(ident)) if *ident == "true" => {
                visitor.visit_bool(true)
            }
            Some(TokenTree::Ident(ident)) if *ident == "false" => {
                visitor.visit_bool(false)
            }
            Some(TokenTree::Ident(ident)) => {
                visitor.visit_string(ident.to_string())
            }
            Some(tt @ TokenTree::Literal(_)) => {
                match syn::parse2::<ExprLit>(TokenStream::from(tt.clone())) {
                    Ok(ExprLit { lit: Lit::Str(s), .. }) => {
                        visitor.visit_string(s.value())
                    }
                    Ok(ExprLit { lit: Lit::Byte(_), .. }) => todo!("byte"),
                    Ok(ExprLit { lit: Lit::Char(ch), .. }) => {
                        visitor.visit_char(ch.value())
                    }
                    Ok(ExprLit { lit: Lit::Int(i), .. }) => {
                        visitor.visit_u64(i.base10_parse::<u64>().or_else(
                            |_| self.deserialize_error(token, "an integer"),
                        )?)
                    }
                    Ok(ExprLit { lit: Lit::Float(f), .. }) => visitor
                        .visit_f64(f.base10_parse::<f64>().or_else(|_| {
                            self.deserialize_error(token, "a float")
                        })?),
                    Ok(ExprLit { lit: Lit::Bool(_), .. }) => {
                        unreachable!("bool is handled elsewhere");
                    }
                    Ok(expr @ ExprLit { .. }) => {
                        todo!(
                            "unhandled expr {}",
                            expr.to_token_stream().to_string(),
                        );
                    }
                    Err(err) => {
                        unreachable!("must be parseable: {} {}", tt, err)
                    }
                }
            }

            Some(neg @ TokenTree::Punct(p)) if p.as_char() == '-' => {
                if let Some(next) = self.next() {
                    let stream = [neg, &next]
                        .into_iter()
                        .cloned()
                        .collect::<TokenStream>();
                    match syn::parse2::<ExprLit>(stream) {
                        Ok(ExprLit { lit: Lit::Int(i), .. }) => {
                            return visitor.visit_i64(
                                i.base10_parse::<i64>().or_else(|_| {
                                    self.deserialize_error(token, "an integer")
                                })?,
                            );
                        }
                        Ok(ExprLit { lit: Lit::Float(f), .. }) => {
                            return visitor.visit_f64(
                                f.base10_parse::<f64>().or_else(|_| {
                                    self.deserialize_error(token, "a float")
                                })?,
                            );
                        }
                        _ => (),
                    }
                }

                self.deserialize_error(token, "a value")
            }
            Some(TokenTree::Punct(_)) => {
                self.deserialize_error(token, "a value")
            }
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_int(|value| visitor.visit_i8(value))
    }
    fn deserialize_i16<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_int(|value| visitor.visit_i16(value))
    }
    fn deserialize_i32<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_int(|value| visitor.visit_i32(value))
    }
    fn deserialize_i64<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_int(|value| visitor.visit_i64(value))
    }
    fn deserialize_i128<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_int(|value| visitor.visit_i128(value))
    }
    fn deserialize_u8<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_int(|value| visitor.visit_u8(value))
    }
    fn deserialize_u16<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_int(|value| visitor.visit_u16(value))
    }
    fn deserialize_u32<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_int(|value| visitor.visit_u32(value))
    }
    fn deserialize_u64<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_int(|value| visitor.visit_u64(value))
    }
    fn deserialize_u128<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_int(|value| visitor.visit_u128(value))
    }

    fn deserialize_f32<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_float(|value| visitor.visit_f32(value))
    }
    fn deserialize_f64<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_float(|value| visitor.visit_f64(value))
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        if !self.pending_member {
            // If this isn't the direct value of an unexpected key, then just
            // process the value.
            self.deserialize_any(visitor)
        } else {
            // If we're expecting a member, this is going to be an error that we
            // want to highlight. We'll identify this error and any error we may
            // encounter while processing the value.

            // Save the token corresponding to the member name.
            let keytok = match self.last.clone() {
                Some(token) => token,
                // This can't happen -- we need to have read a token in order
                // for serde to determine that this value will
                // be ignored.
                None => TokenTree::Group(self.enclosing.clone()),
            };

            // We know this is going to be an error, but we parse the value
            // anyways to see if that *also* produces an error we can report to
            // the user.
            let value = self.deserialize_any(visitor);

            let msg = format!("extraneous member `{}`", &keytok);

            // Create a dummy token stream that contains the token corresponding
            // to the key and the last token we read. Error::new_spanned() just
            // looks at the first and last tokens in the stream.
            let mut ts = TokenStream::new();
            ts.extend(vec![keytok.clone(), self.previous().unwrap()]);

            // Create an error that underlines key, =, and value.
            let mut err = spanned_error(ts, msg);

            // Add in the value error if there was one.
            if let Err(e2) = value {
                err.combine(e2.into_error(&keytok));
            }

            Err(InternalError::Spanned(err))
        }
    }

    // This isn't really attempting to deserialize bytes -- it merely acts as a
    // signal to pass through a TokenStream unperturbed. See the comment on
    // `WrapperVisitor` in `ibidem.rs` for more information.
    fn deserialize_bytes<V>(self, visitor: V) -> InternalResult<V::Value>
    where
        V: Visitor<'de>,
    {
        let next = self.next();

        // TODO format a spanned error of some sort
        let mut token = match &next {
            None => {
                return self.deserialize_error(next, "a value");
            }
            Some(TokenTree::Punct(punct)) if punct.as_char() == ',' => {
                return self.deserialize_error(next, "a value");
            }
            Some(TokenTree::Punct(punct)) if punct.as_char() == '=' => {
                return self.deserialize_error(next, "a value");
            }
            Some(token) => token.clone(),
        };

        // Gather the tokens up to the next ',', '=', or EOF.
        let mut tokens = Vec::new();
        loop {
            tokens.push(token);

            token = match self.peek() {
                None => break,
                Some(TokenTree::Punct(punct)) if punct.as_char() == ',' => {
                    break;
                }
                Some(TokenTree::Punct(punct)) if punct.as_char() == '=' => {
                    break;
                }
                Some(_) => self.next().unwrap(),
            };
        }

        set_wrapper_tokens(tokens);
        visitor.visit_bytes(&[])
    }

    de_unimp!(deserialize_byte_buf);
    de_unimp!(deserialize_unit_struct, _name: &'static str);
    de_unimp!(deserialize_newtype_struct, _name: &'static str);
    de_unimp!(deserialize_tuple_struct, _name: &'static str, _len: usize);
}

#[cfg(test)]
mod tests {
    use crate::{ParseWrapper, ibidem::TokenStreamWrapper};

    use super::*;
    use quote::{ToTokens, quote};
    use std::collections::HashMap;

    #[derive(Clone, Debug, Deserialize)]
    #[serde(untagged)]
    #[allow(dead_code)]
    enum MapEntry {
        Value(String),
        Struct(MapData),
        Array(Vec<MapEntry>),
    }

    type MapData = HashMap<String, MapEntry>;

    fn compare_kv(k: Option<&MapEntry>, v: &str) {
        match k {
            Some(MapEntry::Value(s)) => assert_eq!(s, v),
            _ => panic!("incorrect value"),
        }
    }

    #[test]
    fn simple_map1() -> Result<()> {
        let data = from_tokenstream::<MapData>(&quote! {
            "potato" = potato
        })?;

        compare_kv(data.get("potato"), "potato");

        Ok(())
    }

    #[test]
    fn simple_map2() -> Result<()> {
        let data = from_tokenstream::<MapData>(&quote! {
            "potato" = potato,
            lizzie = "lizzie",
            brickley = brickley,
            "bug" = "bug"
        })?;

        compare_kv(data.get("potato"), "potato");
        compare_kv(data.get("lizzie"), "lizzie");
        compare_kv(data.get("brickley"), "brickley");
        compare_kv(data.get("bug"), "bug");

        Ok(())
    }

    #[test]
    fn bad_ident() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            potato: String,
        }
        match from_tokenstream::<Test>(&quote! {
            "potato" = potato
        }) {
            Err(err) => {
                assert_eq!(
                    err.to_string(),
                    "expected an identifier, but found `\"potato\"`"
                );
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn just_ident() {
        match from_tokenstream::<MapData>(&quote! {
            howdy
        }) {
            Err(msg) => {
                assert_eq!(msg.to_string(), "expected `=` following `howdy`")
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn no_equals() {
        match from_tokenstream::<MapData>(&quote! {
            hi there
        }) {
            Err(msg) => {
                assert_eq!(msg.to_string(), "expected `=`, but found `there`")
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn paren_grouping() {
        match from_tokenstream::<MapData>(&quote! {
            hi = ()
        }) {
            Err(msg) => assert_eq!(
                msg.to_string(),
                "data did not match any variant of untagged enum MapEntry"
            ),
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn no_value() {
        match from_tokenstream::<MapData>(&quote! {
            x =
        }) {
            Err(msg) => {
                assert_eq!(msg.to_string(), "expected a value following `=`")
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn no_value2() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            x: String,
        }
        match from_tokenstream::<Test>(&quote! {
            x =
        }) {
            Err(msg) => {
                assert_eq!(msg.to_string(), "expected a string following `=`")
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn simple() {
        #[derive(Deserialize)]
        struct Test {
            hi: String,
        }
        let m = from_tokenstream::<Test>(&quote! {
            hi = there
        })
        .unwrap();

        assert_eq!(m.hi, "there");
    }

    #[test]
    fn simple2() {
        #[derive(Deserialize)]
        struct Test {
            message: String,
        }
        let m = from_tokenstream::<Test>(&quote! {
            message = "hi there"
        })
        .unwrap();
        assert_eq!(m.message, "hi there");
    }
    #[test]
    fn trailing_comma() {
        #[derive(Deserialize)]
        struct Test {
            hi: String,
        }
        let m = from_tokenstream::<Test>(&quote! {
            hi = there,
        })
        .unwrap();
        assert_eq!(m.hi, "there");
    }

    #[test]
    fn double_comma() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            hi: String,
        }
        match from_tokenstream::<Test>(&quote! {
            hi = there,,
        }) {
            Err(msg) => {
                assert_eq!(
                    msg.to_string(),
                    "expected an identifier, but found `,`"
                );
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn bad_value() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            wat: String,
        }
        match from_tokenstream::<Test>(&quote! {
            wat = ?
        }) {
            Err(msg) => {
                assert_eq!(msg.to_string(), "expected a string, but found `?`");
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn bad_value2() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            the_meaning_of_life_the_universe_and_everything: String,
        }
        match from_tokenstream::<Test>(&quote! {
            the_meaning_of_life_the_universe_and_everything = 42
        }) {
            Err(msg) => {
                assert_eq!(
                    msg.to_string(),
                    "expected a string, but found `42`"
                );
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn bad_value3() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            a: (),
        }
        match from_tokenstream::<Test>(&quote! {
            a = 7,
        }) {
            Err(msg) => {
                assert_eq!(msg.to_string(), "expected a unit, but found `7`");
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn bad_map_value() {
        match from_tokenstream::<MapData>(&quote! {
            wtf = [ ?! ]
        }) {
            Err(msg) => {
                assert_eq!(msg.to_string(), "expected a value, but found `?`")
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn extra_member1() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            a: String,
        }
        match from_tokenstream::<Test>(&quote! {
            b = 42,
            a = "howdy",
        }) {
            Err(msg) => assert_eq!(msg.to_string(), "extraneous member `b`"),
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn extra_member2() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            a: String,
        }
        match from_tokenstream::<Test>(&quote! {
            b = ?,
            a = "howdy",
        }) {
            Err(msg) => {
                assert_eq!(msg.to_string(), "extraneous member `b`");
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn simple_array1() {
        #[derive(Deserialize)]
        struct Test {
            array: Vec<u32>,
        }
        let t = from_tokenstream::<Test>(&quote! {
            array = []
        })
        .unwrap();
        assert!(t.array.is_empty());
    }

    #[test]
    fn simple_array2() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            array: Vec<u32>,
        }
        match from_tokenstream::<Test>(&quote! {
            array = [1, 2, 3]
        }) {
            Ok(t) => assert_eq!(t.array[0], 1),
            Err(err) => panic!("unexpected failure: {:?}", err),
        }
    }

    #[test]
    fn simple_array3() {
        #[derive(Deserialize)]
        struct Test {
            array: Vec<Test2>,
        }
        #[derive(Deserialize)]
        struct Test2 {}
        let t = from_tokenstream::<Test>(&quote! {
            array = [{}]
        })
        .unwrap();
        assert_eq!(t.array.len(), 1);
    }

    #[test]
    fn simple_array4() {
        #[derive(Deserialize)]
        struct Test {
            array: Vec<Test2>,
        }
        #[derive(Deserialize)]
        struct Test2 {}
        let t = from_tokenstream::<Test>(&quote! {
            array = [{}, {},]
        })
        .unwrap();
        assert_eq!(t.array.len(), 2);
    }

    #[test]
    fn bad_array1() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            array: Vec<Test2>,
        }
        #[derive(Deserialize)]
        struct Test2 {}
        match from_tokenstream::<Test>(&quote! {
            array = [{}<-]
        }) {
            Err(msg) => {
                assert_eq!(
                    msg.to_string(),
                    "expected `,` or nothing, but found `<`"
                );
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn bad_array2() {
        match from_tokenstream::<MapData>(&quote! {
            array = [{}<-]
        }) {
            Err(msg) => {
                assert_eq!(
                    msg.to_string(),
                    "expected `,` or nothing, but found `<`"
                );
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn bad_array3() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            array: Vec<Test2>,
        }
        #[derive(Deserialize)]
        struct Test2 {}
        match from_tokenstream::<Test>(&quote! {
            array = {}
        }) {
            Err(msg) => {
                assert_eq!(
                    msg.to_string(),
                    "expected an array, but found `{ }`"
                );
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn bad_array4() {
        match from_tokenstream::<MapData>(&quote! {
            array = [,]
        }) {
            Err(msg) => {
                assert_eq!(msg.to_string(), "expected a value, but found `,`");
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn bad_enum() {
        #[derive(Deserialize)]
        enum Foo {
            Foo,
        }
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            foo: Foo,
        }
        match from_tokenstream::<Test>(&quote! {
            foo = Foop
        }) {
            Err(msg) => {
                assert_eq!(
                    msg.to_string(),
                    "unknown variant `Foop`, expected `Foo`"
                );
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn bad_enum2() {
        #[derive(Deserialize)]
        enum Foo {
            Foo,
        }
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            foo: Foo,
        }
        match from_tokenstream::<Test>(&quote! {
            foo =
        }) {
            Err(msg) => {
                assert_eq!(
                    msg.to_string(),
                    "expected an identifier following `=`"
                );
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn bad_enum3() {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        enum Foo {
            Foo(u32),
        }
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            foo: Foo,
        }
        match from_tokenstream::<Test>(&quote! {
            foo = Foo
        }) {
            Err(msg) => {
                assert_eq!(msg.to_string(), "expected ( following `Foo`");
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn bad_enum4() {
        #[derive(Deserialize)]
        enum Foo {
            #[allow(dead_code)]
            Foo { foo: u32 },
        }
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            foo: Foo,
        }
        match from_tokenstream::<Test>(&quote! {
            foo = Foo
        }) {
            Err(msg) => {
                assert_eq!(msg.to_string(), "expected { following `Foo`");
            }
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn tuple() {
        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            tup: (u32, u32),
        }
        match from_tokenstream::<Test>(&quote! {
            tup = (1, 2)
        }) {
            Ok(t) => assert_eq!(t.tup.1, 2),
            Err(err) => panic!("unexpected failure: {:?}", err),
        }
    }

    fn make_null_group() -> TokenStream {
        // If a consumer uses macro_rules! to specify an expression it will be
        // enclosed in a group with the None delimiter -- effectively an
        // invisible grouping. The constructs that case.
        let group = proc_macro2::Group::new(
            proc_macro2::Delimiter::None,
            quote! {
                "some string"
            },
        );

        quote! {
            s = #group
        }
    }

    #[test]
    fn null_group() {
        #[derive(Deserialize)]
        struct Test {
            s: String,
        }
        match from_tokenstream::<Test>(&make_null_group()) {
            Ok(t) => assert_eq!(t.s, "some string"),
            Err(err) => panic!("unexpected failure: {:?}", err),
        }
    }

    #[test]
    fn null_group_map() -> Result<()> {
        let data = from_tokenstream::<MapData>(&make_null_group())?;

        compare_kv(data.get("s"), "some string");

        Ok(())
    }

    #[test]
    fn int_as_float() {
        #[derive(Deserialize)]
        struct Test {
            x: f64,
        }
        match from_tokenstream::<Test>(&quote! {
            x = 100
        }) {
            Ok(t) => assert_eq!(t.x, 100.0),
            Err(err) => panic!("unexpected failure: {:?}", err),
        };
    }

    #[test]
    fn extra_member_and_bad_value1() {
        #[derive(Deserialize)]
        struct Test {}
        match from_tokenstream::<Test>(&quote! {
            family = ["homer", "marge", "bart", "lisa", "maggie",,]
        }) {
            Err(err) => {
                let errs = err.into_iter().collect::<Vec<_>>();
                assert_eq!(errs[0].to_string(), "extraneous member `family`");
                assert_eq!(
                    errs[1].to_string(),
                    "expected a value, but found `,`"
                );
            }
            Ok(_) => panic!("unexpected success"),
        };
    }

    #[test]
    fn test_numbers() {
        #[derive(Deserialize)]
        #[allow(unused)]
        struct Test {
            pos_i: u64,
            neg_i: i32,
            pos_f: f32,
            neg_f: f64,
        }
        let v = from_tokenstream::<Test>(&quote! {
            pos_i = 0x42,
            neg_i = -2147483648,
            pos_f = 1_000,
            neg_f = -10.0,
        })
        .unwrap();
        assert_eq!(v.pos_i, 0x42);
        assert_eq!(v.neg_i, -2147483648);
        assert_eq!(v.pos_f, 1000.0);
        assert_eq!(v.neg_f, -10.0);

        #[derive(Deserialize)]
        #[allow(unused)]
        struct FlatTest {
            #[serde(flatten)]
            test: Test,
        }
        let v = from_tokenstream::<FlatTest>(&quote! {
            pos_i = 42,
            neg_i = -2147483648,
            pos_f = 1e11,
            neg_f = -1e101,
        })
        .unwrap();
        assert_eq!(v.test.pos_i, 42);
        assert_eq!(v.test.neg_i, -2147483648);
        assert_eq!(v.test.pos_f, 1e11);
        assert_eq!(v.test.neg_f, -1e101);
    }

    enum ClosureOrPath {
        Closure(syn::ExprClosure),
        Path(syn::Path),
    }

    impl syn::parse::Parse for ClosureOrPath {
        fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
            let lookahead = input.lookahead1();

            if lookahead.peek(syn::token::Paren) {
                let group: proc_macro2::Group = input.parse()?;
                return syn::parse2::<Self>(group.stream());
            }

            if let Ok(closure) = input.parse::<syn::ExprClosure>() {
                return Ok(Self::Closure(closure));
            }

            input.parse::<syn::Path>().map(Self::Path)
        }
    }

    impl ClosureOrPath {
        fn to_token_stream(&self) -> proc_macro2::TokenStream {
            match self {
                ClosureOrPath::Closure(c) => c.to_token_stream(),
                ClosureOrPath::Path(p) => p.to_token_stream(),
            }
        }
    }

    #[test]
    fn test_token_stream_wrapper() {
        #[derive(Deserialize)]
        struct Stuff {
            pre_tokens: ParseWrapper<ClosureOrPath>,
            text: String,
            post_tokens: Option<ParseWrapper<ClosureOrPath>>,
            no_tokens: Option<TokenStreamWrapper>,
            things: Vec<ParseWrapper<syn::Path>>,
        }

        let Stuff { pre_tokens, text, post_tokens, no_tokens, things } =
            from_tokenstream::<Stuff>(&quote! {
                text = "howdy",
                pre_tokens = (|a, b, c, d| { let _ = todo!(); }),
                post_tokens = word,
                things = [ serde::Serialize, JsonSchema ],
            })
            .unwrap();

        assert_eq!(
            pre_tokens.to_token_stream().to_string(),
            quote! { |a, b, c, d| { let _ = todo!(); } }.to_string()
        );
        assert_eq!(text, "howdy");
        assert_eq!(
            post_tokens.unwrap().to_token_stream().to_string(),
            quote! { word }.to_string()
        );
        assert!(no_tokens.is_none());
        assert_eq!(
            things[0].to_token_stream().to_string(),
            quote! { serde::Serialize }.to_string()
        );
        assert_eq!(
            things[1].to_token_stream().to_string(),
            quote! { JsonSchema }.to_string()
        );
    }

    #[test]
    fn test_enum() {
        #![allow(dead_code)]

        #[derive(Debug, Deserialize, PartialEq, Eq)]
        enum Thing {
            A,
            B(String),
            C(String, String),
            D { d: String },
        }

        #[derive(Debug, Deserialize)]
        struct Things {
            thing: Thing,
        }

        let a = from_tokenstream::<Things>(&quote! {
            thing = A,
        })
        .unwrap();
        assert_eq!(a.thing, Thing::A);

        let b = from_tokenstream::<Things>(&quote! {
            thing = B("b"),
        })
        .unwrap();
        assert_eq!(b.thing, Thing::B("b".to_string()));

        let c = from_tokenstream::<Things>(&quote! {
            thing = C("cc", "ccc"),
        })
        .unwrap();
        assert_eq!(c.thing, Thing::C("cc".to_string(), "ccc".to_string()));

        let d = from_tokenstream::<Things>(&quote! {
            thing = D { d = "d" },
        })
        .unwrap();
        assert_eq!(d.thing, Thing::D { d: "d".to_string() });
    }

    // Make sure ParseWrapper<syn::Type> is Hash
    #[test]
    fn test_parse_wrapper_hash() {
        #[derive(Deserialize)]
        struct Input {
            #[allow(dead_code)]
            patch: HashMap<ParseWrapper<syn::Ident>, Value>,
        }
        #[derive(Deserialize)]
        struct Value {
            #[allow(dead_code)]
            value: String,
        }

        let _ = from_tokenstream::<Input>(&quote! {
            patch = {
                A = { value = "a" },
                B = { value = "b" },
            }
        })
        .unwrap();
    }

    #[test]
    fn test_parse_wrapper_default_and_clone() {
        fn default_ident() -> ParseWrapper<syn::Ident> {
            ParseWrapper::from(syn::Ident::new(
                "fallback",
                proc_macro2::Span::call_site(),
            ))
        }

        #[derive(Deserialize, Clone)]
        struct Input {
            #[serde(default = "default_ident")]
            ident: ParseWrapper<syn::Ident>,
        }

        let input = from_tokenstream::<Input>(&quote! {}).unwrap();
        assert_eq!(*input.clone().ident, "fallback");

        let input =
            from_tokenstream::<Input>(&quote! { ident = given }).unwrap();
        assert_eq!(*input.ident, "given");

        #[derive(Deserialize)]
        struct Generic {
            #[serde(default)]
            generics: ParseWrapper<syn::Generics>,
        }

        let input = from_tokenstream::<Generic>(&quote! {}).unwrap();
        assert!(input.generics.params.is_empty());

        let input =
            from_tokenstream::<Generic>(&quote! { generics = <T: Copy> })
                .unwrap();
        assert_eq!(input.generics.params.len(), 1);
    }

    #[test]
    fn test_token_stream_wrapper_default_and_clone() {
        #[derive(Deserialize, Clone)]
        struct Input {
            #[serde(default)]
            tokens: TokenStreamWrapper,
        }

        let input = from_tokenstream::<Input>(&quote! {}).unwrap();
        assert!(input.clone().tokens.is_empty());

        let input =
            from_tokenstream::<Input>(&quote! { tokens = a + b }).unwrap();
        assert_eq!(input.tokens.to_string(), "a + b");

        let wrapper = TokenStreamWrapper::from(quote! { x });
        assert_eq!(wrapper.into_inner().to_string(), "x");
    }

    #[test]
    fn test_parse_wrapper_buffered() {
        // In situations with internal buffering, we must produce an error
        // rather than panic.
        #[derive(Deserialize)]
        #[serde(untagged)]
        #[allow(dead_code)]
        enum Untagged {
            I(ParseWrapper<syn::Ident>),
            N(u32),
        }

        #[derive(Deserialize)]
        struct Test {
            #[allow(dead_code)]
            u: Untagged,
        }

        match from_tokenstream::<Test>(&quote! { u = s }) {
            // With untagged, the error produced by us gets swallowed.
            Err(err) => assert_eq!(
                err.to_string(),
                "data did not match any variant of untagged enum Untagged"
            ),
            Ok(_) => panic!("unexpected success"),
        }

        // With flatten, we can produce a helpful message.
        #[derive(Deserialize)]
        struct Inner {
            #[allow(dead_code)]
            id: ParseWrapper<syn::Ident>,
        }

        #[derive(Deserialize)]
        struct Flat {
            #[allow(dead_code)]
            n: u32,
            #[serde(flatten)]
            #[allow(dead_code)]
            inner: Inner,
        }

        match from_tokenstream::<Flat>(&quote! { n = 1, id = s }) {
            Err(err) => assert_eq!(
                err.to_string(),
                "invalid type: string \"s\", expected a ParseWrapper or \
                 TokenStreamWrapper value; these cannot be used inside \
                 `#[serde(flatten)]`, `#[serde(untagged)]`, or similar -- \
                 this is a bug in the macro"
            ),
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn parse_u128() {
        #[derive(Deserialize)]
        struct Test {
            a: u128,
            b: u128,
            c: u128,
        }

        let t = from_tokenstream::<Test>(&quote! {
            a = 0,
            b = 18446744073709551616,
            c = 340282366920938463463374607431768211455,
        })
        .unwrap();
        assert_eq!(t.a, 0);
        assert_eq!(t.b, 18446744073709551616);
        assert_eq!(t.c, 340282366920938463463374607431768211455);
    }

    #[test]
    fn parse_i128() {
        #[derive(Deserialize)]
        struct Test {
            a: i128,
            b: i128,
            c: i128,
            d: i128,
        }

        let t = from_tokenstream::<Test>(&quote! {
            a = -170141183460469231731687303715884105728,
            b = -9223372036854775809,
            c = 9223372036854775808,
            d = 170141183460469231731687303715884105727,
        })
        .unwrap();
        assert_eq!(t.a, -170141183460469231731687303715884105728);
        assert_eq!(t.b, -9223372036854775809);
        assert_eq!(t.c, 9223372036854775808);
        assert_eq!(t.d, 170141183460469231731687303715884105727);
    }

    /// Builds a Delimiter::None group, emulating what rustc does for
    /// macro_rules substitutions.
    fn none_group(inner: TokenStream) -> TokenTree {
        TokenTree::Group(Group::new(Delimiter::None, inner))
    }

    #[test]
    fn test_none_group_nested() {
        #[derive(Deserialize, Debug)]
        struct Test {
            s: String,
        }

        let nested = |inner: TokenStream| {
            none_group(TokenStream::from(none_group(inner)))
        };

        let mut tokens = quote! { s = };
        tokens.extend([nested(quote! { hello })]);
        assert_eq!(from_tokenstream::<Test>(&tokens).unwrap().s, "hello");

        let mut tokens = quote! { s = };
        tokens.extend([nested(quote! {})]);
        assert_eq!(
            from_tokenstream::<Test>(&tokens).unwrap_err().to_string(),
            "expected a string following `=`"
        );

        let mut tokens = quote! { s = };
        tokens.extend([nested(quote! { foo::bar })]);
        assert_eq!(
            from_tokenstream::<Test>(&tokens).unwrap_err().to_string(),
            "expected only a string, but found `foo :: bar`"
        );
    }

    #[test]
    fn test_none_group_any() {
        #[derive(Deserialize, Debug, PartialEq)]
        #[serde(untagged)]
        enum Value {
            S(String),
            N(u32),
        }

        #[derive(Deserialize, Debug)]
        struct Test {
            v: Value,
        }

        // A single value is transparent.
        let mut tokens = quote! { v = };
        tokens.extend([none_group(quote! { hello })]);
        assert_eq!(
            from_tokenstream::<Test>(&tokens).unwrap().v,
            Value::S("hello".to_string())
        );
        let mut tokens = quote! { v = };
        tokens.extend([none_group(quote! { 5 })]);
        assert_eq!(from_tokenstream::<Test>(&tokens).unwrap().v, Value::N(5));

        // An empty group is a missing value.
        let mut tokens = quote! { v = };
        tokens.extend([none_group(quote! {})]);
        assert_eq!(
            from_tokenstream::<Test>(&tokens).unwrap_err().to_string(),
            "expected a value following `=`"
        );

        // More than one value is an error (don't just truncate it to the
        // first!)
        let mut tokens = quote! { v = };
        tokens.extend([none_group(quote! { foo::bar })]);
        assert_eq!(
            from_tokenstream::<Test>(&tokens).unwrap_err().to_string(),
            "expected only a value, but found `foo :: bar`"
        );
    }

    #[test]
    fn test_none_group_kinds() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Inner {
            x: u32,
        }

        #[derive(Deserialize, Debug, PartialEq)]
        enum Kind {
            A,
            B(u32),
        }

        #[derive(Deserialize, Debug, PartialEq)]
        struct Test {
            b: bool,
            n: i64,
            f: f32,
            c: char,
            u: (),
            t: (u32, bool),
            v: Vec<u32>,
            s: Inner,
            k: Kind,
            m: std::collections::BTreeMap<String, u32>,
            o: Option<String>,
        }

        let mut tokens = TokenStream::new();
        for (key, value) in [
            ("b", quote! { true }),
            ("n", quote! { -5 }),
            ("f", quote! { 1.5 }),
            ("c", quote! { 'c' }),
            ("u", quote! { () }),
            ("t", quote! { (1, false) }),
            ("v", quote! { [1, 2] }),
            ("s", quote! { { x = 3 } }),
            ("k", quote! { B(4) }),
            ("m", quote! { { a = 1 } }),
            ("o", quote! { "some" }),
        ] {
            let key =
                proc_macro2::Ident::new(key, proc_macro2::Span::call_site());
            tokens
                .extend([none_group(TokenStream::from(TokenTree::Ident(key)))]);
            tokens.extend(quote! { = });
            tokens.extend([none_group(value)]);
            tokens.extend(quote! { , });
        }
        let test = from_tokenstream::<Test>(&tokens).unwrap();
        assert_eq!(
            test,
            Test {
                b: true,
                n: -5,
                f: 1.5,
                c: 'c',
                u: (),
                t: (1, false),
                v: vec![1, 2],
                s: Inner { x: 3 },
                k: Kind::B(4),
                m: [("a".to_string(), 1)].into_iter().collect(),
                o: Some("some".to_string()),
            }
        );

        #[derive(Deserialize, Debug)]
        struct Just {
            k: Kind,
        }

        let mut tokens = quote! { k = };
        tokens.extend([none_group(quote! { A })]);
        assert_eq!(from_tokenstream::<Just>(&tokens).unwrap().k, Kind::A);

        let mut tokens = quote! { k = };
        tokens.extend([none_group(quote! {})]);
        assert_eq!(
            from_tokenstream::<Just>(&tokens).unwrap_err().to_string(),
            "expected an identifier following `=`"
        );
        let mut tokens = quote! { k = };
        tokens.extend([none_group(quote! { A B })]);
        assert_eq!(
            from_tokenstream::<Just>(&tokens).unwrap_err().to_string(),
            "expected only a variant of `Kind`, but found `A B`"
        );
    }
}
