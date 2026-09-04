// Copyright 2026 Oxide Computer Company

// An empty macro_rules substitution into a TokenStreamWrapper must produce a
// missing-value error.

use testlib::tokens;

macro_rules! wrap {
    ($v:vis ,) => {
        #[tokens {
            tokens = $v,
        }]
        fn test() {}
    };
}

wrap!(,);

fn main() {}
