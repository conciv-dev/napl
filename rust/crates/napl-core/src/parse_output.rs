//! Stage1 adapter over the NAPL-generated `parse_output` crate.

pub use gen_parse_output::extract_yaml;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_yaml_fence() {
        let text = "```yaml\nmodule: greeting\ntests: []\n```\n";
        assert_eq!(extract_yaml(text), "module: greeting\ntests: []");
    }

    #[test]
    fn extracts_bare_fence() {
        assert_eq!(extract_yaml("```\n[]\n```"), "[]");
    }

    #[test]
    fn falls_back_to_trimmed_text() {
        assert_eq!(extract_yaml("  module: x  "), "module: x");
    }
}
