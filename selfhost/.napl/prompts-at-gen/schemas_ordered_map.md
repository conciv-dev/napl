# An insertion-order-preserving string-keyed map

This module provides a small generic map type that remembers the order in which
keys were first inserted, rather than sorting them or using hash order. Some of
the toolchain's on-disk formats depend on a stable, insertion-ordered listing of
string keys (for example the order files are listed for a module), so a plain
hash map or a sorted map would both be wrong here. It is pure: no I/O and no
dependencies on other project modules. Bring in `serde` (and, for the tests,
`serde_json`) since the type must serialize as a map.

## Where this code lives

The working directory is a Cargo workspace whose root manifest is written and
owned by the toolchain — leave it alone. Create this module as its own member
crate in a subdirectory named `schemas_ordered_map/`:
`schemas_ordered_map/Cargo.toml` (package name `schemas_ordered_map`) and
`schemas_ordered_map/src/lib.rs`. Touch nothing outside `schemas_ordered_map/`.
Ensure `cargo test` passes from the workspace root before finishing.

## The type

Expose a public generic type `OrderedMap<V>` — a map from owned `String` keys to
values of type `V`. It preserves **first-insertion order**: iterating the map,
its keys, or its values always visits entries in the order their keys were first
inserted. Provide a `Default` implementation (an empty map) and a `new()`
constructor that returns the same empty map.

## Query and mutation methods

- `len()` — the number of entries.
- `is_empty()` — whether there are no entries.
- `contains_key(key: &str)` — whether the key is present.
- `get(key: &str)` — a shared reference to the value for a key, or nothing.
- `get_mut(key: &str)` — a mutable reference to the value for a key, or nothing.
- `insert(key: String, value: V)` — set the value for a key. If the key is
  already present, replace its value **in place**, keeping the key at its
  existing position; otherwise append it at the end (a new last entry).
- `remove(key: &str)` — remove a key, returning its value if it was present
  (otherwise nothing), and closing the gap so the remaining keys keep their
  relative order.
- `iter()` — visit `(&String, &V)` pairs in insertion order.
- `keys()` — visit `&String` keys in insertion order.
- `values()` — visit `&V` values in insertion order.

## Worked behavior to reproduce exactly

- Insert `z`, then `a`, then `m`; the keys iterate as `z`, `a`, `m` — insertion
  order, not sorted order.
- Insert `a` = 1, `b` = 2, then `a` = 9; `get("a")` is `9` and the keys are still
  `a`, `b` (re-inserting `a` did not move it to the end).
- Insert `a` = 1, `b` = 2, then remove `a`; the removal returns the value `1`,
  the map no longer contains `a`, and the remaining key is `b`.

## Serialization

Implement `serde::Serialize` and `serde::Deserialize` for `OrderedMap<V>` so it
serializes as an ordinary map (object), emitting its entries in insertion order,
and deserializes from a map by reading entries in the order they appear, keeping
that order. With a JSON serializer, an `OrderedMap` built by inserting `z` = 1
then `a` = 2 serializes to exactly the string `{"z":1,"a":2}` (note: `z` before
`a`), and deserializing that string back yields a map equal to the original.
Support equality comparison (`PartialEq`) so two maps with the same entries in
the same order compare equal.
