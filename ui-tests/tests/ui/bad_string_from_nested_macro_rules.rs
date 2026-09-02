// Copyright 2026 Oxide Computer Company

// We must handle nested macro_rules substitutions the same way we handle
// single-level substitutions.

use testlib::annotation;

macro_rules! inner {
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

macro_rules! outer {
    ($s:expr) => {
        inner!($s);
    };
}

outer!(123);

fn main() {}
