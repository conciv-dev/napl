//! Prompt/machine extension alias logic with codepoint-correct matching.

/// The canonical prompt file extension.
pub const PROMPT_EXTENSION: &str = ".napl";
/// The canonical machine (compiled) file extension.
pub const MACHINE_EXTENSION: &str = ".mapl";
/// The emoji machine-file alias.
pub const MACHINE_ALIAS: &str = ".\u{1F916}";

/// The curated single-person emoji prompt aliases.
pub const DEFAULT_PROMPT_ALIASES: [&str; 6] = [
    ".\u{1F9D1}",
    ".\u{1F9D3}",
    ".\u{1F464}",
    ".\u{1F468}",
    ".\u{1F469}",
    ".\u{1F9D2}",
];

/// The curated aliases as owned strings.
#[must_use]
pub fn default_prompt_aliases() -> Vec<String> {
    DEFAULT_PROMPT_ALIASES
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

/// The recognized prompt extensions: the canonical spelling plus `aliases`
/// (the curated list when `aliases` is `None`).
#[must_use]
pub fn prompt_extensions(aliases: Option<&[String]>) -> Vec<String> {
    let mut out = vec![PROMPT_EXTENSION.to_string()];
    match aliases {
        Some(list) => out.extend(list.iter().cloned()),
        None => out.extend(default_prompt_aliases()),
    }
    out
}

/// The recognized machine extensions: canonical plus emoji alias.
#[must_use]
pub fn machine_extensions() -> Vec<String> {
    vec![MACHINE_EXTENSION.to_string(), MACHINE_ALIAS.to_string()]
}

/// Whether `path` is a prompt file, matching any recognized extension by
/// (codepoint-correct) suffix.
#[must_use]
pub fn is_prompt_file(path: &str, aliases: Option<&[String]>) -> bool {
    prompt_extensions(aliases)
        .iter()
        .any(|ext| path.ends_with(ext))
}

/// Whether `path` is a machine file.
#[must_use]
pub fn is_machine_file(path: &str) -> bool {
    machine_extensions().iter().any(|ext| path.ends_with(ext))
}

/// The machine extension mirroring a prompt's spelling: a canonical `.napl`
/// prompt keeps `.mapl`, any other (emoji) prompt gets the emoji alias.
#[must_use]
pub fn machine_extension_for_prompt(prompt_path: &str) -> &'static str {
    if prompt_path.ends_with(PROMPT_EXTENSION) {
        MACHINE_EXTENSION
    } else {
        MACHINE_ALIAS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_canonical_and_every_curated_alias() {
        assert!(is_prompt_file(&format!("greeting{PROMPT_EXTENSION}"), None));
        for alias in DEFAULT_PROMPT_ALIASES {
            assert!(is_prompt_file(&format!("greeting{alias}"), None));
        }
    }

    #[test]
    fn matches_multibyte_emoji_by_codepoint() {
        assert!("greeting.\u{1F9D1}".ends_with(".\u{1F9D1}"));
        assert!(is_prompt_file("a/b/c/person.\u{1F9D3}", None));
        assert!(!is_prompt_file("robot.\u{1F916}", None));
    }

    #[test]
    fn rejects_zwj_sequence_not_on_curated_list() {
        // ".👨‍💻" = man + ZWJ + laptop, not a curated single-person alias.
        assert!(!is_prompt_file("team.\u{1F468}\u{200D}\u{1F4BB}", None));
        assert!(!DEFAULT_PROMPT_ALIASES.contains(&".\u{1F468}\u{200D}\u{1F4BB}"));
    }

    #[test]
    fn prompt_extensions_defaults_to_canonical_plus_curated() {
        let mut expected = vec![PROMPT_EXTENSION.to_string()];
        expected.extend(default_prompt_aliases());
        assert_eq!(prompt_extensions(None), expected);
    }

    #[test]
    fn prompt_extensions_honours_override() {
        assert_eq!(
            prompt_extensions(Some(&[".\u{1F9D1}".to_string()])),
            vec![PROMPT_EXTENSION.to_string(), ".\u{1F9D1}".to_string()]
        );
        let override_list = [".\u{1F9D1}".to_string()];
        assert!(!is_prompt_file("greeting.\u{1F9D3}", Some(&override_list)));
        assert!(is_prompt_file("greeting.\u{1F9D1}", Some(&override_list)));
    }

    #[test]
    fn recognizes_both_machine_spellings() {
        assert_eq!(
            machine_extensions(),
            vec![MACHINE_EXTENSION.to_string(), MACHINE_ALIAS.to_string()]
        );
        assert!(is_machine_file(&format!("greeting{MACHINE_EXTENSION}")));
        assert!(is_machine_file("greeting.\u{1F916}"));
        assert!(!is_machine_file("greeting.napl"));
    }

    #[test]
    fn mirrors_prompt_spelling_for_machine_extension() {
        assert_eq!(
            machine_extension_for_prompt("examples/greeting.napl"),
            MACHINE_EXTENSION
        );
        for alias in DEFAULT_PROMPT_ALIASES {
            assert_eq!(
                machine_extension_for_prompt(&format!("examples/greeting{alias}")),
                MACHINE_ALIAS
            );
        }
    }
}
