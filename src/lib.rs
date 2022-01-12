// Copyright 2022 Oxide Computer Company

//! This is a `serde::Deserializer` implementation for
//! `proc_macro2::TokenStream`. It is intended for proc_macro builders who want
//! rich configuration in their custom attributes.
//!
//! If you'd like the consumers of your macro use it like this:
//!
//! ```ignore
//! #[my_macro {
//!     settings = {
//!         reticulate_splines = true,
//!         normalizing_power = false,
//!     },
//!     disaster = "pandemic",
//! }]
//! ```
//!
//! Your macro will start like this:
//!
//! ```ignore
//! #[proc_macro_attribute]
//! pub fn my_macro(
//!     attr: proc_macro::TokenStream,
//!     item: proc_macro::TokenStream,
//! ) -> proc_macro::TokenStream {
//!     // ...
//! # }
//! ```
//!
//! Use `serde_tokenstream` to deserialize `attr` into a structure with the
//! `Deserialize` trait (typically via a `derive` macro):
//!
//! ```
//! # use proc_macro2::TokenStream;
//! # use serde_tokenstream::from_tokenstream;
//! # use serde::Deserialize;
//! # #[derive(Deserialize)]
//! # struct Config;
//! # pub fn my_macro(
//! #     attr: proc_macro2::TokenStream,
//! #     item: proc_macro2::TokenStream,
//! # ) -> proc_macro2::TokenStream {
//! let config = match from_tokenstream::<Config>(&TokenStream::from(attr)) {
//!     Ok(c) => c,
//!     Err(err) => return err.to_compile_error().into(),
//! };
//! # item
//! # }
//! ```

mod ibidem;
mod serde_tokenstream;

pub use crate::ibidem::ParseWrapper;
pub use crate::ibidem::TokenStreamWrapper;
pub use crate::serde_tokenstream::from_tokenstream;
pub use crate::serde_tokenstream::Error;
pub use crate::serde_tokenstream::Result;
