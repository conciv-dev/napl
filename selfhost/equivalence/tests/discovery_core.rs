use discovery_core::{declared_crate, find_duplicate_module, module_paths_from};

#[test]
fn declared_crate_reads_the_key_or_none() {
    assert_eq!(
        declared_crate("---\nmodule: solo\ncrate: shared\n---\nBody.\n").as_deref(),
        Some("shared")
    );
    assert_eq!(declared_crate("---\nmodule: solo\n---\nBody.\n"), None);
    assert_eq!(declared_crate("no frontmatter here"), None);
}

#[test]
fn find_duplicate_module_reports_the_first_repeat() {
    let files = vec![
        ("---\nmodule: a\n---\n".to_string(), "a.napl".to_string()),
        ("---\nmodule: b\n---\n".to_string(), "sub/b.napl".to_string()),
        ("---\nmodule: a\n---\n".to_string(), "copy/a.napl".to_string()),
    ];
    assert_eq!(
        find_duplicate_module(&files).as_deref(),
        Some("duplicate module 'a' is declared by two prompts: a.napl and copy/a.napl. Copied a prompt? Rename the module in the copy.")
    );
}

#[test]
fn find_duplicate_module_none_when_unique_and_skips_unparseable() {
    let files = vec![
        ("---\nmodule: a\n---\n".to_string(), "a.napl".to_string()),
        ("garbage".to_string(), "bad.napl".to_string()),
        ("---\nmodule: b\n---\n".to_string(), "b.napl".to_string()),
    ];
    assert_eq!(find_duplicate_module(&files), None);
}

#[test]
fn module_paths_from_builds_the_module_index() {
    let files = vec![
        ("---\nmodule: a\n---\n".to_string(), "a.napl".to_string()),
        ("garbage".to_string(), "bad.napl".to_string()),
        ("---\nmodule: b\n---\n".to_string(), "sub/b.napl".to_string()),
    ];
    let paths = module_paths_from(&files);
    assert_eq!(paths.get("a").map(String::as_str), Some("a.napl"));
    assert_eq!(paths.get("b").map(String::as_str), Some("sub/b.napl"));
    assert_eq!(paths.len(), 2);
}
