//! Equivalence gate for the `schemas::ordered_map` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core`
//! `schemas::ordered_map` module (rust/crates/napl-core/src/schemas/ordered_map.rs),
//! replayed against the NAPL-generated `schemas_ordered_map` crate under
//! selfhost/.napl/src/rust/schemas_ordered_map/. Each case asserts the same
//! input -> output the hand-written module asserts for itself.

use schemas_ordered_map::OrderedMap;

#[test]
fn preserves_insertion_order() {
    let mut m: OrderedMap<i32> = OrderedMap::new();
    m.insert("z".to_string(), 1);
    m.insert("a".to_string(), 2);
    m.insert("m".to_string(), 3);
    assert_eq!(m.keys().cloned().collect::<Vec<_>>(), vec!["z", "a", "m"]);
}

#[test]
fn insert_replaces_in_place() {
    let mut m: OrderedMap<i32> = OrderedMap::new();
    m.insert("a".to_string(), 1);
    m.insert("b".to_string(), 2);
    m.insert("a".to_string(), 9);
    assert_eq!(m.get("a"), Some(&9));
    assert_eq!(m.keys().cloned().collect::<Vec<_>>(), vec!["a", "b"]);
}

#[test]
fn remove_deletes() {
    let mut m: OrderedMap<i32> = OrderedMap::new();
    m.insert("a".to_string(), 1);
    m.insert("b".to_string(), 2);
    assert_eq!(m.remove("a"), Some(1));
    assert!(!m.contains_key("a"));
    assert_eq!(m.keys().cloned().collect::<Vec<_>>(), vec!["b"]);
}

#[test]
fn json_roundtrip_preserves_order() {
    let mut m: OrderedMap<i32> = OrderedMap::new();
    m.insert("z".to_string(), 1);
    m.insert("a".to_string(), 2);
    let json = serde_json::to_string(&m).unwrap();
    assert_eq!(json, r#"{"z":1,"a":2}"#);
    let back: OrderedMap<i32> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, m);
}
