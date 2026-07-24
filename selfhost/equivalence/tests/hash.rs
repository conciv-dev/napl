//! Equivalence gate for the `hash` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core` `hash`
//! module (rust/crates/napl-core/src/hash.rs), replayed against the
//! NAPL-generated `hash` crate under selfhost/.napl/src/rust/hash/. Each case
//! asserts the same input -> output the hand-written module asserts for itself.

use hash::content_hash;

#[test]
fn deterministic_for_identical_content() {
    assert_eq!(content_hash("hello"), content_hash("hello"));
}

#[test]
fn differs_for_different_content() {
    assert_ne!(content_hash("hello"), content_hash("world"));
}

#[test]
fn produces_64_char_hex() {
    let h = content_hash("x");
    assert_eq!(h.len(), 64);
    assert!(h
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

#[test]
fn matches_known_sha256_vector() {
    assert_eq!(
        content_hash("hello"),
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
}
