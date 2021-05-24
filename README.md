# ordbog

## Ordbog

This is a small crate providing a single special-purpose lossy compresison
code, designed for use as a "scan accelerator" for database storage. Such
codes are not replacements for underlying values; rather they provide cheap
approximate answers to predicates that may be sufficient to elide accessing
the underlying data, similar to the way bloom filters can elide lookups, but
supporting more general predicates (eg. tabulations, range-queries).

Put another way: rewriting a query on the underlying data values to a query
on codes can produce false positives -- requiring a secondary query of the
underlying data -- but no false negatives. And for half the codes in a given
dictionary (the "exact" codes, assigned to high-frequency inputs), they also
do not produce false positives.

The codes are "cheap" (i.e. actually useful for acceleration) for three
reasons:

  1. They are small, so conserve memory bandwidth: 1 or 2 bytes per code,
     vs. 8 bytes for an underlying float/u64 value, or more for a string,
     high resolution timestamp, uuid or large-decimal type.

  2. They are simple integers, where the underlying data may be something
     more costly to process.

  3. They are SIMD-friendly: an AVX2 scan can look at 16 or 32 codes at a
     time, and a GPU scan can look at hundreds at a time.

The crate is equally usable for numeric, textual or categorical data. All it
needs is something ordered. It includes wrapper types for floating point.

The codes it produces have the following characteristics:

  1. Each code value is logically 8 or 16 bits (depending on the `Mode`
     enum). The user decides whether to operate with 8 or 16 bits: 8 bit
     codes should be used for memory-only scans, to elide 64-byte cache-line
     accesses; 16 bit codes should be used for disk scans, to elide 4k page
     accesses.

  2. Code value 0 is unused, so that subsequent compression can use it as a
     sentinel or missing-value code.

  3. All other codes alternate between even/exact (representing a specific
     value in the input) and odd/inexact (representing an open interval of
     possible input values). Values 1 and 0xff (or 0xffff, or whatever the
     final odd code is in the dictionary) thus encode one-sided lower and
     upper open intervals.

  4. Codes are assigned to cover a _sample_ provided by the user, which is
     internally sorted and then partitioned into equal-sized bins, including
     duplicates. Then each run of duplicates within a bin is counted. The
     sample value with the longest run -- i.e. the highest-frequency sample
     value -- within each bin is given an (even) exact code. Then an (odd)
     inexact code is given to each open interval of sample values between
     sample values that were given exact codes. The provided sample should
     therefore be big enough to be representative of the total input; but if
     it is not representative, encoding still works, it just loses
     efficiency.

  5. The assigned codes imply order and preserve equality, specifically:
       - `code(a) < code(b)` implies `a < b`
       - `a < b` implies `code(a) <= code(b)`
       - `a == b` implies `code(a) == code(b)`

### Reference

Brian Hentschel, Michael S. Kester, and Stratos Idreos. 2018. Column
Sketches: A Scan Accelerator for Rapid and Robust Predicate Evaluation. In
Proceedings of the 2018 International Conference on Management of Data
(SIGMOD '18). Association for Computing Machinery, New York, NY, USA,
857–872.

DOI: <https://doi.org/10.1145/3183713.3196911>

<https://stratos.seas.harvard.edu/files/stratos/files/sketches.pdf>

### Name

Wikitionary (Danish):

> Noun: ordbog (singular definite ordbogen, plural indefinite ordbøger)
> 1. dictionary, lexicon
>
> Etymology: From ord ("word") +‎ bog ("book"). Compare Swedish ordbok,
> English wordbook, German Wörterbuch.

License: MIT OR Apache-2.0
