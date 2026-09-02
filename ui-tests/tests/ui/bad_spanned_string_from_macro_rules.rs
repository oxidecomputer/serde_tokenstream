// Copyright 2026 Oxide Computer Company

// A SpannedString provided a non-string substitution must report a diagnostic
// at the invocation site, not at the declaration site.

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

wrap!(123);

fn main() {}
