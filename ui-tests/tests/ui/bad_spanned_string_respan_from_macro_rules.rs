// Copyright 2026 Oxide Computer Company

// Like bad_spanned_string_respan, with the value provided by a macro_rules
// substitution.

use testlib::annotation;

macro_rules! wrap {
    ($s:expr) => {
        #[annotation {
            string = "test",
            options = OptionA,
            unit = (),
            tup = (1, 2.0),
            ident = $s,
        }]
        fn test() {}
    };
}

wrap!("undefined_thing");

fn main() {}
