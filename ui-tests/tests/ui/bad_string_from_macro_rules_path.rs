// Copyright 2026 Oxide Computer Company

// A multi-token macro_rules substitution must be rejected, not truncated to its
// first token.

use testlib::annotation;

macro_rules! wrap {
    ($s:expr) => {
        #[annotation {
            string = $s,
            options = OptionA,
            unit = (),
            tup = (1, 2.0),
        }]
        fn test() {}
    };
}

wrap!(foo::bar);

fn main() {}
