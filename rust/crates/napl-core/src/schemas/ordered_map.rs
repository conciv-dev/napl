//! A minimal insertion-order-preserving string-keyed map.
//!
//! The TS map/attribution logic relies on JS object insertion order (e.g. the
//! file listing order in `filesForModule`). A `HashMap` loses it and a
//! `BTreeMap` reorders lexicographically, so this preserves insertion order the
//! way the port requires, without pulling in an external ordered-map crate.

use std::fmt;
use std::marker::PhantomData;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An ordered string-keyed map preserving insertion order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedMap<V> {
    entries: Vec<(String, V)>,
}

impl<V> Default for OrderedMap<V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<V> OrderedMap<V> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Insert or replace a value, preserving the position of an existing key.
    pub fn insert(&mut self, key: String, value: V) {
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    /// Remove a key, returning its value if present.
    pub fn remove(&mut self, key: &str) -> Option<V> {
        let idx = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.remove(idx).1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &V)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, v)| v)
    }
}

impl<V: Serialize> Serialize for OrderedMap<V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.entries.len()))?;
        for (k, v) in &self.entries {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for OrderedMap<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapVisitor<V>(PhantomData<V>);
        impl<'de, V: Deserialize<'de>> Visitor<'de> for MapVisitor<V> {
            type Value = OrderedMap<V>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some((key, value)) = access.next_entry::<String, V>()? {
                    entries.push((key, value));
                }
                Ok(OrderedMap { entries })
            }
        }
        deserializer.deserialize_map(MapVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
