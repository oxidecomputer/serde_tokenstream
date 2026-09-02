// Copyright 2026 Oxide Computer Company

// An empty macro_rules substitution into a ParseWrapper must produce a
// missing-value error, similar to plain values.

use testlib::annotation;

macro_rules! wrap {
    ($v:vis ,) => {
        #[annotation {
            string = "test",
            options = OptionA,
            unit = (),
            tup = (1, 2.0),
            bool_expr = $v,
        }]
        fn test() {}
    };
}

wrap!(,);

fn main() {}
