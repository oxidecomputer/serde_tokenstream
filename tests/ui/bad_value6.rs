// Copyright 2020 Oxide Computer Company

use testlib::annotation;

#[annotation {
    many = ["1 potato", "2 potato", 3]
}]
fn test() {}

fn main() {}
