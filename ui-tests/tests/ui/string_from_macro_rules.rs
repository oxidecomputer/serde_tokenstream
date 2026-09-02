// Copyright 2026 Oxide Computer Company

// With macro_rules substitutions, rustc will wrap the substitutions inside a
// `Delimiter::None` group. Ensure that such groups are accepted. (In this
// test, the error produced must be for the later `unit` field.)

use testlib::annotation;

macro_rules! wrap {
    ($s:expr) => {
        #[annotation {
            string = $s,
            options = OptionA,
            unit = 1,
            tup = (1, 2.0),
        }]
        fn test() {}
    };
}

wrap!(foo);

fn main() {}
