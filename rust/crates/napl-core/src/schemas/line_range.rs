//! Stage1 adapter over the NAPL-generated `schemas_line_range` crate.

pub use schemas_line_range::LineRange;

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
