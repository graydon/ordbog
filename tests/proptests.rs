// Copyright 2021 Graydon Hoare <graydon@pobox.com>
// Licensed under the MIT and Apache-2.0 licenses.

use float_next_after::NextAfter;
use float_ord::FloatOrd;
use ordbog::{Dict, DictF64, Mode};
use proptest::collection::*;
use proptest::prelude::*;
use proptest::sample::Index;
use std::fmt::Debug;
use std::num::FpCategory;

trait Testable: Ord + Clone + Default + Debug {
    fn check_pair(d: &Dict<Self>, a: &Self, b: &Self) {
        let c0 = d.encode(a);
        let c1 = d.encode(b);
        assert!(c0 <= d.mode.max_inexact_code());
        assert!(c1 <= d.mode.max_inexact_code());
        if c0 < c1 {
            assert!(*a < *b);
        }
        if *a == *b {
            assert!(c0 == c1);
        }
        if *a < *b {
            assert!(c0 <= c1);
        }
    }

    fn check_triple(d: &Dict<Self>, a: &Self, b: &Self, c: &Self) {
        let c0 = d.encode(a);
        let c1 = d.encode(b);
        let c2 = d.encode(c);
        assert!(c0 <= d.mode.max_inexact_code());
        assert!(c1 <= d.mode.max_inexact_code());
        assert!(c2 <= d.mode.max_inexact_code());

        Self::check_pair(d, a, b);
        Self::check_pair(d, a, c);
        Self::check_pair(d, b, c);
        assert!(c0 <= c1);
        assert!(c1 <= c2);
        if c0.is_exact() {
            assert!(c0 < c1);
            assert!(c0 < c2);
        }
        if c1.is_exact() {
            assert!(c0 < c1);
            assert!(c1 < c2);
        }
        if c2.is_exact() {
            assert!(c0 < c2);
            assert!(c1 < c2);
        }
    }

    fn next(x: &Self) -> Option<Self>;
    fn prev(x: &Self) -> Option<Self>;

    fn check_next_and_prev(d: &Dict<Self>, x: &Self) {
        let prev = Self::prev(x);
        let next = Self::next(x);
        match (prev, next) {
            (Some(p), None) => Self::check_pair(d, &p, x),
            (None, Some(q)) => Self::check_pair(d, x, &q),
            (Some(p), Some(q)) => Self::check_triple(d, &p, x, &q),
            (None, None) => (),
        }
    }

    fn check_dict_of_sample(sample: Vec<Self>) {
        for mode in vec![Mode::Byte, Mode::Word] {
            let d: Dict<Self> = Dict::new(mode, sample.clone());
            assert!(d.codes.len() <= mode.num_exact_codes());
            for slice in d.codes.windows(2) {
                assert!(slice[0] != slice[1]);
                assert!(slice[0] < slice[1]);
            }
            for s in sample.iter() {
                Self::check_next_and_prev(&d, s);
            }
            for a in sample.iter().rev().take(10).chain(sample.iter().take(10)) {
                for b in sample.iter().rev().take(10).chain(sample.iter().take(10)) {
                    Self::check_pair(&d, a, b);
                }
            }
        }
    }
}

impl Testable for i32 {
    fn next(x: &Self) -> Option<Self> {
        if *x < std::i32::MAX {
            Some(*x + 1)
        } else {
            None
        }
    }
    fn prev(x: &Self) -> Option<Self> {
        if *x > std::i32::MIN {
            Some(*x - 1)
        } else {
            None
        }
    }
}

impl Testable for String {
    fn next(x: &Self) -> Option<Self> {
        let mut n = x.clone();
        n.push('a');
        Some(n)
    }
    fn prev(x: &Self) -> Option<Self> {
        if x.is_empty() {
            None
        } else {
            let mut n = x.clone();
            n.pop();
            Some(n)
        }
    }
}

impl Testable for DictF64 {
    // FloatOrd order is:
    //
    // NaN | -Infinity | x < 0 | -0 | +0 | x > 0 | +Infinity | NaN
    //
    fn next(x: &Self) -> Option<Self> {
        let DictF64(FloatOrd(v)) = *x;
        let fopt = match v {
            a if a.classify() == FpCategory::Nan && a.is_sign_positive() => None,
            a if a.classify() == FpCategory::Nan && a.is_sign_negative() => Some(f64::NEG_INFINITY),
            a if a == f64::INFINITY => Some(f64::NAN),
            a if a == f64::NEG_INFINITY => Some(f64::MIN),
            a if a == f64::MAX => Some(f64::INFINITY),
            a if a == -0.0 && a.is_sign_negative() => Some(0.0),
            other => Some(other.next_after(f64::INFINITY)),
        };
        fopt.map(|f| DictF64(FloatOrd::<f64>(f)))
    }
    fn prev(x: &Self) -> Option<Self> {
        let DictF64(FloatOrd(v)) = *x;
        let fopt = match v {
            a if a.classify() == FpCategory::Nan && a.is_sign_negative() => None,
            a if a.classify() == FpCategory::Nan && a.is_sign_positive() => Some(f64::INFINITY),
            a if a == f64::NEG_INFINITY => Some(-f64::NAN),
            a if a == f64::INFINITY => Some(f64::MAX),
            a if a == 0.0 && a.is_sign_positive() => Some(-0.0),
            other => Some(other.next_after(f64::NEG_INFINITY)),
        };
        fopt.map(|f| DictF64(FloatOrd::<f64>(f)))
    }
}

proptest! {
    // TODO: add a test that does some statistical distribution checking (normal, uniform, zipf)
    #[test]
    fn integer_dict(sample in vec(any::<i32>(), 0..100000)) {
        <i32 as Testable>::check_dict_of_sample(sample);
    }

    // Strings are quite a bit slower so we don't push into the full range of word mode
    #[test]
    fn string_dict(sample in vec(any::<String>(), 0..1000)) {
        <String as Testable>::check_dict_of_sample(sample);
    }

    #[test]
    fn float_dict(floats in vec(any::<f64>(), 1..100000),
                  indices in vec(any::<Index>(), 0..100000)
)
    {
        let sample : Vec<f64> = indices.iter().map(|ix| floats[ix.index(floats.len())]).collect();
        let sample : Vec<DictF64> = sample.iter().map(|f| DictF64(FloatOrd(*f))).collect();
        <DictF64 as Testable>::check_dict_of_sample(sample);
    }
}
