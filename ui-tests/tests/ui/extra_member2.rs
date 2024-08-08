// Copyright 2020 Oxide Computer Company

use testlib::annotation;

#[annotation {
    family = ["homer", "marge", "bart", "lisa", "maggie",,]
}]
fn test() {}

fn main() {}
