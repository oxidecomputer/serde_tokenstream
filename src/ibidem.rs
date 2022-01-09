// Copyright 2022 Oxide Computer Company

use proc_macro2::{TokenStream, TokenTree};
use serde::{de::Error, de::Visitor, Deserialize};

/// A Wrapper around proc_macro2::TokenStream that is Deserializable, albeit
/// only in the context of from_tokenstream(). You can use this if, say, your
/// macro allows users to pass in Rust tokens as a configuration option. This
/// can be useful, for example, in a macro that generates code where the caller
/// of that macro might want to augment the generated code.
#[derive(Debug)]
pub struct TokenStreamWrapper(pub TokenStream);

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

// A wrapper around the syn::parse::Parse trait that is Deserializable, albeit
// only in the context of from_tokenstream(). This extends [TokenStreamWrapper]
// by further interpreting the TokenStream and guiding the user in the case of
// parse errors.
#[derive(Debug)]
pub struct ParseWrapper<P: syn::parse::Parse>(pub P);

impl<'de, P: syn::parse::Parse> Deserialize<'de> for ParseWrapper<P> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let token_stream = deserializer.deserialize_bytes(WrapperVisitor)?;

        Ok(Self(
            syn::parse2::<P>(token_stream).map_err(|_| {
                D::Error::custom("TODO: pass through error info")
            })?,
        ))
    }
}
impl<P: syn::parse::Parse> std::ops::Deref for ParseWrapper<P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// While it is convenient to be able to "pass through" TokenStreams
/// unperturbed, or to interpret TokenStreams via syn::parse::Parse.
/// While serde--wisely--does not permit this kind of unholy communion between
/// Deserialize and Deserializer, we can skirt around this with the
/// otherwise-unused deserialize_bytes/visit_bytes interfaces. Since there is
/// no TokenStream that could reasonably be interpreted as bytes, we use this
/// interface as a conduit to pass through a serialized form of a portion of
/// the TokenStream from our Deserializer::deserialize_bytes() and deserialize
/// it in WrapperVisitor::visit_bytes(). Yes, there is a serializer/deserializer
/// pair buried *within* the broader deserialization. Gotta spend money to make
/// money.
struct WrapperVisitor;

impl<'de> Visitor<'de> for WrapperVisitor {
    type Value = TokenStream;

    fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        formatter.write_str("TokenStream")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        deserialize_token_stream(v)
    }
}

/// We turn the tokens into text and return them as bytes.
pub(crate) fn serialize_token_stream(tokens: Vec<TokenTree>) -> Vec<u8> {
    tokens.into_iter().collect::<TokenStream>().to_string().bytes().collect()
}

/// We parse the bytes back into a TokenStream; it would be surprising if this
/// failed.
fn deserialize_token_stream<E: serde::de::Error>(
    v: &[u8],
) -> Result<TokenStream, E> {
    String::from_utf8(v.to_vec())
        .unwrap()
        .parse()
        .map_err(|_| E::custom("parse error"))
}
