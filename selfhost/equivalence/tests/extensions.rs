//! Equivalence gate for the `extensions` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core`
//! `extensions` module (rust/crates/napl-core/src/extensions.rs), replayed
//! against the NAPL-generated `extensions` crate under
//! selfhost/.napl/src/rust/extensions/. Each case asserts the same input ->
//! output the hand-written module asserts for itself. The generated
//! `prompt_extensions`/`is_prompt_file` take `Option<&[&str]>` where the
//! hand-written ones take `Option<&[String]>`, and `machine_extensions` returns
//! `&'static str` slices where the hand-written one returns owned `String`s; the
//! observable values compared are identical, which is the point — equivalence is
//! behavioral, not type-identical.

use extensions::{
    default_prompt_aliases, is_machine_file, is_prompt_file, machine_extension_for_prompt,
    machine_extensions, prompt_extensions, CURATED_PROMPT_ALIASES, MACHINE_ALIAS,
    MACHINE_EXTENSION, PROMPT_EXTENSION,
};

#[test]
fn recognizes_canonical_and_every_curated_alias() {
    assert!(is_prompt_file(&format!("greeting{PROMPT_EXTENSION}"), None));
    for alias in CURATED_PROMPT_ALIASES {
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
    assert!(!CURATED_PROMPT_ALIASES.contains(&".\u{1F468}\u{200D}\u{1F4BB}"));
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
        prompt_extensions(Some(&[".\u{1F9D1}"])),
        vec![PROMPT_EXTENSION.to_string(), ".\u{1F9D1}".to_string()]
    );
    let override_list = [".\u{1F9D1}"];
    assert!(!is_prompt_file("greeting.\u{1F9D3}", Some(&override_list)));
    assert!(is_prompt_file("greeting.\u{1F9D1}", Some(&override_list)));
}

#[test]
fn recognizes_both_machine_spellings() {
    assert_eq!(machine_extensions(), vec![MACHINE_EXTENSION, MACHINE_ALIAS]);
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
    for alias in CURATED_PROMPT_ALIASES {
        assert_eq!(
            machine_extension_for_prompt(&format!("examples/greeting{alias}")),
            MACHINE_ALIAS
        );
    }
}
