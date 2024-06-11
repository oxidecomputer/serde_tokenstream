// Copyright 2020 Oxide Computer Company

use testlib::outer;

#[outer]
#[annotation {
    nested = {
        gosling = 0.0,
        eyas = 0,
    }
}]
fn test() {}

fn main() {}
