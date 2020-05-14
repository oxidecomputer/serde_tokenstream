# `serde_tokenstream`
This Rust crate is intended for use with macros that need bespoke configuration.
It's implemented as a `serde::Deserializer` that operates on a
`proc_macro2::TokenSteam` (easily converted from the standard
`proc_macro::TokenStream`).

## Usage

Say we're building a custom proc macro that you want consumers to use like
this:

```rust
#[MyMacro {
    name = "SNPP",
    owner = "Canary M Burns",
    details = {
        type = Fission,
        year_of_opening = 1968,
    }
}]
fn some_func() {
    ...
}
```

The function that implements the proc macro must have two parameters (both of
type `proc_macro::TokenStream`): attributes (the tokens with the braces that
follow the name of the macro), and the item (the function, type, etc. to
which the macro is applied):

```rust
#[proc_macro_attribute]
pub fn MyMacro(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    ...
}
```

We'll first define the `struct` type that represents the configuration and
`derive` a `serde::Deserialize`:

```rust
#[derive(Deserialize)]
struct Config {
    name: String,
    owner: String,
    details: ConfigDetails,
}

#[derive(Deserialize)]
struct ConfigDetails {
    type: ConfigDetailsType,
    year_of_opening: usize,
}

#[derive(Deserialize)]
enum ConfigDetailsType {
    Coal,
    Fission,
    Hydroelectric,
}
```

Now we can parse `attr` into the `Config` struct with `serde_tokenstream::from_tokenstream`:

```rust
use proc_macro2::TokenStream;
use serde_tokenstream::from_tokenstream;

#[allow(non_snake_case)]
#[proc_macro_attribute]
pub fn MyMacro(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let config = match from_tokenstream::<Config>(&TokenStream::from(attr)) {
        Ok(c) => c,
        Err(err) => return err.to_compile_error().into(),
    }

    ...
}
```

See the `serde` documentation for the full range of controls that can be
applied to types and their members.

## Error Handling

Note that errors will highlight the problematic portion of consuming code:

```rust
#[MyMacro{
    name = "Rocinante",
    owner = "Rocicorp",
    details = {
        kind = Fusion,
        year_of_opening = 2347
    }
}]
fn deploy() {
    ...
}
```

```
error: unknown variant `Fusion`, expected one of `Coal`, `Fission`, `Hydroelectric`
 --> tests/test_err1.rs:7:16
  |
7 |         kind = Fusion,
  |                ^^^^^^
```
