// Copyright 2021 Graydon Hoare <graydon@pobox.com>
// Licensed under the MIT and Apache-2.0 licenses.

//! # Ordbog
//!
//! This is a small crate providing a single special-purpose lossy compresison
//! code, designed for use as a "scan accelerator" for database storage. Such
//! codes are not replacements for underlying values; rather they provide cheap
//! approximate answers to predicates that may be sufficient to elide accessing
//! the underlying data, similar to the way bloom filters can elide lookups, but
//! supporting more general predicates (eg. tabulations, range-queries).
//!
//! Put another way: rewriting a query on the underlying data values to a query
//! on codes can produce false positives -- requiring a secondary query of the
//! underlying data -- but no false negatives. And for half the codes in a given
//! dictionary (the "exact" codes, assigned to high-frequency inputs), they also
//! do not produce false positives.
//!
//! The codes are "cheap" (i.e. actually useful for acceleration) for three
//! reasons:
//!
//!   1. They are small, so conserve memory bandwidth: 1 or 2 bytes per code,
//!      vs. 8 bytes for an underlying float/u64 value, or more for a string,
//!      high resolution timestamp, uuid or large-decimal type.
//!
//!   2. They are simple integers, where the underlying data may be something
//!      more costly to process.
//!
//!   3. They are SIMD-friendly: an AVX2 scan can look at 16 or 32 codes at a
//!      time, and a GPU scan can look at hundreds at a time.
//!
//! The crate is equally usable for numeric, textual or categorical data. All it
//! needs is something ordered. It includes wrapper types for floating point.
//!
//! The codes it produces have the following characteristics:
//!
//!   1. Each code value is logically 8 or 16 bits (depending on the `Mode`
//!      enum). The user decides whether to operate with 8 or 16 bits: 8 bit
//!      codes should be used for memory-only scans, to elide 64-byte cache-line
//!      accesses; 16 bit codes should be used for disk scans, to elide 4k page
//!      accesses.
//!
//!   2. Code value 0 is unused, so that subsequent compression can use it as a
//!      sentinel or missing-value code.
//!
//!   3. All other codes alternate between even/exact (representing a specific
//!      value in the input) and odd/inexact (representing an open interval of
//!      possible input values). Values 1 and 0xff (or 0xffff, or whatever the
//!      final odd code is in the dictionary) thus encode one-sided lower and
//!      upper open intervals.
//!
//!   4. Codes are assigned to cover a _sample_ provided by the user, which is
//!      internally sorted and then partitioned into equal-sized bins, including
//!      duplicates. Then each run of duplicates within a bin is counted. The
//!      sample value with the longest run -- i.e. the highest-frequency sample
//!      value -- within each bin is given an (even) exact code. Then an (odd)
//!      inexact code is given to each open interval of sample values between
//!      sample values that were given exact codes. The provided sample should
//!      therefore be big enough to be representative of the total input; but if
//!      it is not representative, encoding still works, it just loses
//!      efficiency.
//!
//!   5. The assigned codes imply order and preserve equality, specifically:
//!        - `code(a) < code(b)` implies `a < b`
//!        - `a < b` implies `code(a) <= code(b)`
//!        - `a == b` implies `code(a) == code(b)`
//!
//! ## Reference
//!
//! Brian Hentschel, Michael S. Kester, and Stratos Idreos. 2018. Column
//! Sketches: A Scan Accelerator for Rapid and Robust Predicate Evaluation. In
//! Proceedings of the 2018 International Conference on Management of Data
//! (SIGMOD '18). Association for Computing Machinery, New York, NY, USA,
//! 857–872.
//!
//! DOI: <https://doi.org/10.1145/3183713.3196911>
//!
//! <https://stratos.seas.harvard.edu/files/stratos/files/sketches.pdf>
//!
//! ## Name
//!
//! Wikitionary (Danish):
//!
//! > Noun: ordbog (singular definite ordbogen, plural indefinite ordbøger)
//! > 1. dictionary, lexicon
//! >
//! > Etymology: From ord ("word") +‎ bog ("book"). Compare Swedish ordbok,
//! > English wordbook, German Wörterbuch.

use float_ord::FloatOrd;
use std::fmt::Debug;

/// Wrapper that supplies a Default (1.0) value around [FloatOrd]. This is the
/// type to use for a [Dict] of underlying [f64] values.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct DictF64(pub FloatOrd<f64>);

impl Default for DictF64 {
    fn default() -> Self {
        DictF64(FloatOrd(1.0))
    }
}

/// Wrapper that supplies a Default (1.0) value around [FloatOrd]. This is the
/// type to use for a [Dict] of underlying [f32] values.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct DictF32(pub FloatOrd<f32>);

impl Default for DictF32 {
    fn default() -> Self {
        DictF32(FloatOrd(1.0))
    }
}

/// Wrapper for a [Dict] code value. If the [Dict] was
/// built with [Mode::Byte], this will have values ranging only
/// over `[1,255]`. If the [Dict] was built with [Mode::Word],
/// this will have values ranging over `[1,65535]`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Code(pub u16);
impl Code {

    /// Return true iff the code is an _exact_ code, i.e. a code which
    /// represents a single underlying value rather than a range of possible
    /// values. This is true iff the code is an even number.
    pub fn is_exact(&self) -> bool {
        (self.0 & 1) == 0
    }
}

/// Indicates whether to build a small [Dict] of up to 255 values
/// or a larger one of up to 65535 values.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Mode {
    /// Build a [Dict] with up to 255 codes ranging over `[1,255]`. This mode is
    /// most appropriate when building a sketch that elides accesses to smaller
    /// underlying storage blocks like 64-byte cache lines, where the small
    /// repertoire of codes (and thus higher per-element chance of a false
    /// positive) is offset by the small size of each storage block (and thus
    /// small number of elements).
    Byte,
    /// Build a [Dict] with up to 65535 codes ranging over `[1,65535]`. This
    /// mode is most appropriate when building a sketch that elides access to
    /// larger underlying storage blocks like 4096-byte pages, where the larger
    /// number of elements per storage block demands a comparatively low false
    /// positive probability per element.
    Word,
}
impl Mode {
    /// Returns the count of exact codes in the mode: either `127` for
    /// [Mode::Byte] or `32767` for [Mode::Word].
    pub fn num_exact_codes(&self) -> usize {
        match self {
            Mode::Byte => 127,
            Mode::Word => 32767,
        }
    }
    /// Returns the maximum exact code in the mode: either `0xfe` for
    /// [Mode::Byte] or `0xfffe` for [Mode::Word].
    pub fn max_exact_code(&self) -> Code {
        match self {
            Mode::Byte => Code(0xfe),
            Mode::Word => Code(0xfffe),
        }
    }
    /// Returns the maximum inexact code in the mode: either `0xff` for
    /// [Mode::Byte] or `0xffff` for [Mode::Word].
    pub fn max_inexact_code(&self) -> Code {
        match self {
            Mode::Byte => Code(0xff),
            Mode::Word => Code(0xffff),
        }
    }
}

/// Trait expressing requirements for the types of underlying values
/// that can be encoded in a [Dict].
pub trait ValReq : Ord + Clone + Default /*+ Debug*/ {}
impl<T> ValReq for T where T : Ord + Clone + Default /*+ Debug*/ {}

struct Cluster<T: ValReq> {
    value: T,
    count: usize,
}
/// A dictionary over an underlying type `T` conforming to [ValReq]. The
/// dictionary maps underlying values to [Code]s to use in a sketch, using
/// [Dict::encode].
pub struct Dict<T: ValReq> {

    /// The mode the dictionary was built in.
    pub mode: Mode,

    /// A sorted vector of the values assigned exact codes in the dictionary.
    /// Implicitly defines both exact and inexact code values based on the
    /// positions of exact codes in the vector.
    pub codes: Vec<T>,
}

impl<T: ValReq> Dict<T> {
    fn clusters(sorted_sample: &Vec<T>) -> Vec<Cluster<T>> {
        let mut clu = Vec::with_capacity(sorted_sample.len());
        if !sorted_sample.is_empty() {
            let mut curr = &sorted_sample[0];
            let mut count = 0;
            for i in sorted_sample.iter() {
                if *i == *curr {
                    count += 1;
                } else {
                    let value = (*curr).clone();
                    clu.push(Cluster { value, count });
                    curr = i;
                    count = 1
                }
            }
            let value = (*curr).clone();
            clu.push(Cluster { value, count });
        }
        clu
    }

    /// Look up the code for a value of the underlying value type `T`.
    pub fn encode(&self, query: &T) -> Code {
        // The `self.code` array stores the input values assigned to "exact"
        // codes, counting upwards from code 2. Thus a successful binary search
        // landing at `idx` returns exact code `2*(idx+1)`. An unsuccessful
        // binary search lands on the _next_ exact code greater than the query
        // value, so we subtract 1 from that code to denote the inexact code
        // covering the range below that next exact code.
        let code = match self.codes.binary_search(query) {
            Ok(idx) => 2 * (idx + 1),
            Err(idx) => (2 * (idx + 1)) - 1,
        };
        assert!(code <= 0xffff);
        Code(code as u16)
    }

    fn assign_codes_with_step(codestep: usize, clu: &Vec<Cluster<T>>) -> Vec<T> {
        let mut codes = Vec::new();
        let mut first_idx = 0;
        while first_idx < clu.len() {
            let mut last_idx = first_idx;
            let mut idx_with_max_val = first_idx;
            let mut cluster_count_sum = 0;
            while last_idx < clu.len() && cluster_count_sum < codestep {
                if clu[idx_with_max_val].count < clu[last_idx].count {
                    idx_with_max_val = last_idx;
                }
                cluster_count_sum += clu[last_idx].count;
                last_idx += 1;
            }
            codes.push(clu[idx_with_max_val].value.clone());
            // FIXME: boundary condition might be wrong here, I think?
            // does this skip the end of each cluster? Why is life full
            // of boundary errors? *Sobs* I am such a fool.
            first_idx = last_idx + 1;
        }
        codes
    }

    fn assign_codes_with_minimal_step(
        samplesize: usize,
        ncodes: usize,
        clu: &Vec<Cluster<T>>,
    ) -> Vec<T> {
        assert!(samplesize != 0);
        assert!(ncodes != 0);
        assert!(ncodes < samplesize);

        // Each code should cover at least codestep worth of the sample.
        let mut codestep = samplesize / ncodes;

        /*
        println!(
            "each of {} target codes should span {} samples",
            ncodes, codestep
        );
        */

        // We start with a basic dictionary with each code covering `codestep`
        // sample vaules, calculated by taking elements from the cluster list.
        let mut codes = Self::assign_codes_with_step(codestep, &clu);

        // Unfortunately it's possible some of those clusters overshoot the
        // `codestep`, giving us codes that cover too many sample values and
        // therefore giving us too few overall codes. To correct for this, we
        // want to iterate a few times (up to 8 times -- ad-hoc limit)
        // estimating the error, reducing the `codestep` and re-encoding, to try
        // to get as close as possible (without going over) the target number of
        // codes.
        for _ in 0..=8 {
            assert!(!codes.is_empty());

            // If we hit the target we're done.
            if codes.len() == ncodes {
                break;
            }

            // If we overshot the target, truncate the best attempt and return.
            if codes.len() > ncodes {
                codes.resize(ncodes, <T as Default>::default());
                break;
            }

            // Otherwise estimate, reduce, and (if it's an improvement) accept.
            assert!(codes.len() < ncodes);
            let bias = (codes.len() * 10000) / ncodes;
            codestep *= bias;
            codestep /= 10000;
            /*
            println!(
                "wrong number of codes ({}), adjusting step to {}",
                codes.len(),
                codestep
            );
            */
            let next_codes = Self::assign_codes_with_step(codestep, &clu);
            if next_codes.len() <= ncodes {
                codes = next_codes;
            } else {
                break;
            }
        }
        codes
    }

    /// Build a dictionary with a given [Mode] over a provided sample of the
    /// underlying value type.
    ///
    /// The provided sample should be representative of the overall data, and
    /// specifically should contain duplicates at a similar proportion to those
    /// found in the overall data. If the sample is not representative, code
    /// accuracy will degrade (thus false positives will increase) but nothing
    /// else will go wrong.
    ///
    /// This function will sort the sample, so the sample should be small enough
    /// that the caller can tolerate the running time of sorting it. Otherwise
    /// the larger the sample, the more accurate the codes.
    pub fn new(mode: Mode, mut sample: Vec<T>) -> Self {
        // println!("beginning building dictionary from {} samples", sample.len());

        // For an empty sample we haven't much to work with; assign exact code 2
        // for the default value in the target type. Any value less than default
        // will code as 1, any value greater as 3. That's it.
        if sample.is_empty() {
            // println!("empty sample, using 1-element default");
            let codes = vec![<T as Default>::default()];
            return Self { mode, codes };
        }

        // If we have a real sample, we want to sort it both to assign
        // order-preserving codes and to cluster it for frequency analysis.
        sample.sort_unstable();

        // Do the frequency analysis.
        let clu = Self::clusters(&sample);
        assert!(!clu.is_empty());

        /*
        if clu.len() == sample.len()
        {
            println!("samples are all unique");
        }
        else
        {
            println!("reduced {} samples to {} clusters", sample.len(), clu.len());
        }
        */

        let ncodes = mode.num_exact_codes();

        // If there are the same or fewer clusters than the codespace, we can
        // just assign one code per cluster, there's no need for anything
        // fancier.
        if clu.len() <= ncodes {
            /*
            println!(
                "fewer clusters ({}) than target codes {}, using clusters",
                clu.len(), ncodes);
            */
            let codes = clu.into_iter().map(|c| c.value).collect();
            return Self { mode, codes };
        }
        let codes = Self::assign_codes_with_minimal_step(sample.len(), ncodes, &clu);
        // println!("finished building dictionary with {} exact codes", codes.len());
        Self { mode, codes }
    }
}
