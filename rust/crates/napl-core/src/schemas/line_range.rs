//! `LineRange` mirrors the zod `lineRangeSchema` preprocess + tuple:
//! a bare integer `n` or a single-element array `[n]` normalizes to `[n, n]`;
//! a two-element array `[a, b]` passes through; every element must be an
//! integer `>= 1`; any other shape is rejected.

use std::fmt;

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

/// An inclusive 1-based line range `[start, end]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

impl LineRange {
    #[must_use]
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

fn int_from_f64<E: de::Error>(v: f64) -> Result<u32, E> {
    if v.fract() != 0.0 {
        return Err(E::custom("expected an integer line number"));
    }
    if v < 1.0 || v > f64::from(u32::MAX) {
        return Err(E::custom("line number must be >= 1"));
    }
    Ok(v as u32)
}

fn int_from_u64<E: de::Error>(v: u64) -> Result<u32, E> {
    if v < 1 {
        return Err(E::custom("line number must be >= 1"));
    }
    u32::try_from(v).map_err(|_| E::custom("line number out of range"))
}

fn int_from_i64<E: de::Error>(v: i64) -> Result<u32, E> {
    if v < 1 {
        return Err(E::custom("line number must be >= 1"));
    }
    u32::try_from(v).map_err(|_| E::custom("line number out of range"))
}

struct LineNum(u32);

impl<'de> Deserialize<'de> for LineNum {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NumVisitor;
        impl Visitor<'_> for NumVisitor {
            type Value = LineNum;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an integer >= 1")
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<LineNum, E> {
                int_from_u64(v).map(LineNum)
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<LineNum, E> {
                int_from_i64(v).map(LineNum)
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<LineNum, E> {
                int_from_f64(v).map(LineNum)
            }
        }
        deserializer.deserialize_any(NumVisitor)
    }
}

impl<'de> Deserialize<'de> for LineRange {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RangeVisitor;
        impl<'de> Visitor<'de> for RangeVisitor {
            type Value = LineRange;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an integer or a 1- or 2-element integer array (>= 1)")
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<LineRange, E> {
                let n = int_from_u64(v)?;
                Ok(LineRange::new(n, n))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<LineRange, E> {
                let n = int_from_i64(v)?;
                Ok(LineRange::new(n, n))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<LineRange, E> {
                let n = int_from_f64(v)?;
                Ok(LineRange::new(n, n))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<LineRange, A::Error> {
                let mut nums: Vec<u32> = Vec::new();
                while let Some(LineNum(n)) = seq.next_element()? {
                    nums.push(n);
                }
                match nums.as_slice() {
                    [only] => Ok(LineRange::new(*only, *only)),
                    [a, b] => Ok(LineRange::new(*a, *b)),
                    _ => Err(de::Error::custom("line range must have 1 or 2 elements")),
                }
            }
        }
        deserializer.deserialize_any(RangeVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(v: serde_json::Value) -> Result<LineRange, serde_json::Error> {
        serde_json::from_value(v)
    }

    #[test]
    fn scalar_normalizes_to_pair() {
        assert_eq!(parse(json!(8)).unwrap(), LineRange::new(8, 8));
    }

    #[test]
    fn single_element_array_normalizes() {
        assert_eq!(parse(json!([8])).unwrap(), LineRange::new(8, 8));
    }

    #[test]
    fn two_element_array_passes_through() {
        assert_eq!(parse(json!([5, 7])).unwrap(), LineRange::new(5, 7));
    }

    #[test]
    fn rejects_zero() {
        assert!(parse(json!(0)).is_err());
        assert!(parse(json!([0, 2])).is_err());
    }

    #[test]
    fn rejects_non_integer() {
        assert!(parse(json!(1.5)).is_err());
        assert!(parse(json!([1.5, 2])).is_err());
    }

    #[test]
    fn accepts_integral_float() {
        assert_eq!(parse(json!(2.0)).unwrap(), LineRange::new(2, 2));
    }

    #[test]
    fn rejects_three_element_array() {
        assert!(parse(json!([1, 2, 3])).is_err());
    }

    #[test]
    fn rejects_string() {
        assert!(parse(json!("nope")).is_err());
    }
}
