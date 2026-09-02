// Copyright 2026 Oxide Computer Company

// An empty macro_rules substitution (such as an empty `vis`) must be a
// missing-value error, not a panic.

use testlib::annotation;

macro_rules! wrap {
    ($v:vis ,) => {
        #[annotation {
            string = $v,
            options = OptionA,
            unit = (),
            tup = (1, 2.0),
        }]
        fn test() {}
    };
}

wrap!(,);

fn main() {}
