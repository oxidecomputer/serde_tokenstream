// Copyright 2026 Oxide Computer Company

// An empty macro_rules substitution before a value must be skipped. (The error
// produced must be for the later `maybe_string` field.)

use testlib::annotation;

macro_rules! wrap {
    ($v:vis ,) => {
        #[annotation {
            string = $v foo,
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
