# A 1-based inclusive line range

This module defines a small value type for an inclusive line range together with
a lenient deserializer that accepts several JSON spellings and normalizes them to
one shape. It is pure: no I/O and no dependencies on other project modules. Bring
in `serde` (and, for the tests, `serde_json`).

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `schemas_line_range/`:
`schemas_line_range/Cargo.toml` (package name `schemas_line_range`) and
`schemas_line_range/src/lib.rs`. Touch nothing outside `schemas_line_range/`.
Ensure `cargo test` passes from the workspace root before finishing.

## The type

Expose a public type `LineRange` with two public fields, `start` and `end`, each
an unsigned 32-bit integer, representing an inclusive 1-based line range. Provide
a `new(start, end)` constructor. Support equality comparison and cloning/copying
so two ranges with the same endpoints compare equal.

## Deserializing a range

Implement `serde::Deserialize` for `LineRange` accepting three input shapes and
normalizing them:

- A bare number `n` becomes the range `n..=n` (start and end both `n`).
- A one-element array `[n]` becomes `n..=n`.
- A two-element array `[a, b]` becomes the range with start `a` and end `b`.

Every line number involved must be an **integer that is at least 1**. Enforce
this strictly:

- The number `0` is rejected (line numbers are 1-based), whether it appears bare
  (`0`) or inside an array (`[0, 2]`).
- A number with a nonzero fractional part is rejected, whether bare (`1.5`) or in
  an array (`[1.5, 2]`). A number whose fractional part is zero is accepted and
  treated as that integer — so `2.0` deserializes to the range `2..=2`.
- An array with three or more elements is rejected, and an array with zero
  elements is rejected — only one- and two-element arrays are valid.
- A value of any other type (a string such as `"nope"`, a boolean, an object) is
  rejected.

Rejection means deserialization returns an error, not a panic. The exact wording
of the error does not matter; only which inputs are accepted versus rejected, and
the normalized `start`/`end` for accepted inputs.
