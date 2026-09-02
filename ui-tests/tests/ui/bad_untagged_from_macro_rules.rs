// Copyright 2026 Oxide Computer Company

// When macro_rules substitutions are involved, failures must point at the
// invocation, not at the macro definition.

use testlib::untagged_list;

macro_rules! wrap {
    ($e:expr) => {
        #[untagged_list {
                    items = [$e],
                }]
        fn test() {}
    };
}

wrap!({ c = 1 });

fn main() {}
