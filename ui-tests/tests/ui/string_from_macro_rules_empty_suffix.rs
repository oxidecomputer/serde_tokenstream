// Copyright 2026 Oxide Computer Company

// An macro_rules substitution after a value must be skipped. (The error
// produced must be for the later `maybe_string` field.)

use testlib::annotation;

macro_rules! wrap {
    ($v:vis ,) => {
        #[annotation {
            string = foo $v,
            options = OptionA,
            unit = (),
            tup = (1, 2.0),
            maybe_string = 1,
        }]
        fn test() {}
    };
}

wrap!(,);

fn main() {}
