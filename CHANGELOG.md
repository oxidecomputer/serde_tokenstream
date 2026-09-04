# Changelog

<!-- next-header -->
## Unreleased - ReleaseDate

### Added

- `SpannedString`, a string that remembers its span.

  With `serde_tokenstream`, a `String` field accepts either a string literal or a bare identifier. But the deserialized `String` doesn't track the span it came from. `ParseWrapper<SpannedString>` accepts the same inputs and records the token's span, so a macro that interprets the string further (e.g., as an identifier or a file name) can report errors at the value.

  `SpannedString`'s `parse` and `parse_with` methods parse the value as Rust syntax and attach the span to the result.

- `spanned_error`, a replacement for `syn::Error::new_spanned`. When a `macro_rules!` macro substitutes a value into an attribute, `syn::Error::new_spanned` reports the error at the macro _definition_. `spanned_error` reports it at the _invocation_, where the value was written. The invocation is generally more helpful as a diagnostic.

- `ParseWrapper` and `TokenStreamWrapper` now implement `Clone`, `Default`, and `From`.

### Fixed

- Values that a `macro_rules!` macro substitutes into an attribute are now accepted by every kind of field. rustc wraps each substitution in a transparent group, and previously only untyped values would descend into the group; typed fields such as strings and numbers rejected the value.
- Empty `macro_rules!` substitutions, such as an empty `$v:vis`, no longer cause a panic.
- Fixed a number of bugs around incorrect span attribution. For example, errors raised before any value token was read, as with `Named()` or `value =` at the end of an attribute, previously panicked or pointed at the wrong location. They now point at the nearest available token, such as the `=` or the enclosing delimiters.
- Tokens left over in a newtype variant, tuple variant, or tuple are now rejected rather than silently dropped. For example, previously, `Named("a", "b")` for a newtype variant `Named(String)` would be parsed as `Named("a")`. Now, such cases fail.
- Tokens left over after a `ParseWrapper` value, as in `bool_expr = true false`, now produce a better error message: "expected `,` or nothing, but found `false`". Previously, they produced a bare "unexpected token".
- A missing value, as in `bool_expr = ,`, now reports "expected a value, but found `,`".
- `ParseWrapper` and `TokenStreamWrapper` cannot work in cases where serde performs internal buffering (most commonly, `#[serde(flatten)]` or `#[serde(untagged)]`). The types now produce errors making this limitation clearer.

## [0.3.0] - 2026-07-20

### Changed

- Updated to `syn` 3.0. `ParseWrapper` now requires types that implement `syn` 3.0's `Parse` trait.

- Moved to the Rust 2024 edition. MSRV updated to Rust 1.85.

---

For information about older versions, see the [commit history](https://github.com/oxidecomputer/serde_tokenstream/commits/main/).

<!-- next-url -->
[0.3.0]: https://github.com/oxidecomputer/serde_tokenstream/releases/tag/v0.3.0
