// Copyright 2020 Oxide Computer Company

use testlib::outer;

#[outer]
#[annotation {
    string = "hey"
}]
fn test() {}

fn main() {}
