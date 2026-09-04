// Copyright 2026 Oxide Computer Company

// A macro_rules substitution that is not a string must be reported at the
// invocation, not the declaration.

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

wrap!(123);

fn main() {}
