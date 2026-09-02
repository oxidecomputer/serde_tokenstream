// Copyright 2026 Oxide Computer Company

// Empty macro_rules substitutions around a ParseWrapper value must be skipped.
// (The error produced must be for the later `maybe_string` field.)

use testlib::annotation;

macro_rules! wrap {
    ($v:vis ,) => {
        #[annotation {
            string = "test",
            options = OptionA,
            unit = (),
            tup = (1, 2.0),
            bool_expr = $v true $v,
            maybe_string = 1,
        }]
        fn test() {}
    };
}

wrap!(,);

fn main() {}
