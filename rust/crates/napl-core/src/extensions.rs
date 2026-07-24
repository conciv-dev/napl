//! Stage1 adapter over the NAPL-generated `extensions` crate. The generated
//! `prompt_extensions`/`is_prompt_file` take `Option<&[&str]>` and
//! `machine_extensions` returns `&'static str` slices; the wrappers below bridge
//! those seams back to the hand-written `Option<&[String]>` / `Vec<String>`
//! surface the callers expect.

pub use gen_extensions::{
    default_prompt_aliases, is_machine_file, machine_extension_for_prompt,
    CURATED_PROMPT_ALIASES as DEFAULT_PROMPT_ALIASES, MACHINE_ALIAS, MACHINE_EXTENSION,
    PROMPT_EXTENSION,
};

#[must_use]
pub fn prompt_extensions(aliases: Option<&[String]>) -> Vec<String> {
    let borrowed: Option<Vec<&str>> = aliases.map(|a| a.iter().map(String::as_str).collect());
    gen_extensions::prompt_extensions(borrowed.as_deref())
}

#[must_use]
pub fn machine_extensions() -> Vec<String> {
    gen_extensions::machine_extensions()
        .into_iter()
        .map(String::from)
        .collect()
}

#[must_use]
pub fn is_prompt_file(path: &str, aliases: Option<&[String]>) -> bool {
    let borrowed: Option<Vec<&str>> = aliases.map(|a| a.iter().map(String::as_str).collect());
    gen_extensions::is_prompt_file(path, borrowed.as_deref())
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
