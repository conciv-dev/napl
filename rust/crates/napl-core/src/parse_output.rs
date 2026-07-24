//! Extraction of fenced YAML from an LLM response, mirroring `extractYaml`.

/// Extract the first fenced code block's contents (```` ```yaml ````/```` ```yml ````/
/// bare ```` ``` ````), trimmed; otherwise the whole text, trimmed. Mirrors the
/// regex `/```(?:ya?ml)?[^\n]*\n([\s\S]*?)```/i`.
#[must_use]
pub fn extract_yaml(text: &str) -> String {
    if let Some(fence_start) = text.find("```") {
        let after_fence = &text[fence_start + 3..];
        if let Some(nl) = after_fence.find('\n') {
            let content_start = &after_fence[nl + 1..];
            if let Some(close) = content_start.find("```") {
                return content_start[..close].trim().to_string();
            }
        }
    }
    text.trim().to_string()
}

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
