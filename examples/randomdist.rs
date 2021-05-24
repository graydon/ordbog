// Copyright 2021 Graydon Hoare <graydon@pobox.com>
// Licensed under the MIT and Apache-2.0 licenses.

use float_ord::FloatOrd;
use plotlib::{self, page::Page, repr, style::PointStyle, view::ContinuousView};
use rand;
use rand_distr::{Distribution, Exp1, Normal, Uniform};

use ordbog::{Dict, DictF64, Mode};

fn plot_dist<D: Distribution<f64>>(name: &str, dist: &D) {
    let mut sample: Vec<DictF64> = Vec::new();
    let mut rng = rand::thread_rng();
    for _ in 0..10000 {
        sample.push(DictF64(FloatOrd(dist.sample(&mut rng))));
    }
    let dict = Dict::new(Mode::Byte, sample);
    let data: Vec<(f64, f64)> = dict
        .codes
        .iter()
        .enumerate()
        .map(|(x, y)| (x as f64, y.0 .0))
        .collect();
    let repr = repr::Plot::new(data).point_style(PointStyle::new());
    let view = ContinuousView::new().add(repr);
    let str = Page::single(&view).dimensions(70, 20).to_text().unwrap();
    println!("code assignments of {} data:\n{}", name, str);
}

fn main() {
    let normal = Normal::new(0.0, 2.0).unwrap();
    plot_dist("normal", &normal);

    let uniform = Uniform::new(0.0, 2.0);
    plot_dist("uniform", &uniform);

    let exp = Exp1;
    plot_dist("exp1", &exp);
}
