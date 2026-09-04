// Copyright 2026 Oxide Computer Company

// Test that macro_rules substitutions are accepted for every value kind. (The
// error must be for the later `maybe_string` field.)

use testlib::annotation;

macro_rules! wrap {
    ($o:expr, $u:expr, $t:expr, $m:expr, $n:expr, $g:expr) => {
        #[annotation {
            string = "test",
            options = $o,
            unit = $u,
            tup = $t,
            many = $m,
            nested = { squeaker = "x", eyas = $n, gosling = 1.0 },
            painted = { color = { red = true, green = $g, blue = false } },
            maybe_string = 1,
        }]
        fn test() {}
    };
}

wrap!(OptionA, (), (1, 2.0), ["a", b], 7, false);

fn main() {}
