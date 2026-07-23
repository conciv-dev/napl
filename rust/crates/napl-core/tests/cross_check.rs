//! Cross-validation against fixtures under `rust/fixtures/`.
//!
//! The fixture files were produced by RUNNING the built TypeScript
//! implementation (`packages/*/dist`) — see `rust/README.md`. These tests assert
//! the Rust port reproduces the exact TS outputs and accept/reject decisions.

use std::fs;
use std::path::PathBuf;

use napl_core::blame::{blame_file, BlameSourceEntry};
use napl_core::hash::content_hash;
use napl_core::scanner::{scan_document, DepSource, RegionSpan, Span};
use napl_core::schemas::{
    parse_attribution_entries, parse_lock, parse_map, validate_attribution, validate_ir,
    validate_ml,
};
use napl_core::text_diff::{apply_hunks, parse_hunks, to_lines, unified_diff};
use serde::Deserialize;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
}

fn load(name: &str) -> String {
    fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
}

#[derive(Deserialize)]
struct DiffCase {
    before: String,
    after: String,
    diff: String,
}

#[test]
fn diff_matches_ts() {
    let cases: Vec<DiffCase> = serde_json::from_str(&load("diff_cases.json")).unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        assert_eq!(
            unified_diff(&case.before, &case.after),
            case.diff,
            "diff mismatch for {:?} -> {:?}",
            case.before,
            case.after
        );
        // roundtrip: parsing the TS diff and applying it reproduces the target
        let rebuilt = apply_hunks(&case.before, &parse_hunks(&case.diff));
        assert_eq!(rebuilt, to_lines(&case.after).join("\n"));
    }
}

#[derive(Deserialize)]
struct HashCase {
    input: String,
    hash: String,
}

#[test]
fn hash_matches_ts() {
    let cases: Vec<HashCase> = serde_json::from_str(&load("hash_cases.json")).unwrap();
    for case in cases {
        assert_eq!(content_hash(&case.input), case.hash);
    }
}

#[derive(Deserialize)]
struct BlameEntry {
    gen: i64,
    timestamp: String,
    module: String,
    patch: String,
}

#[derive(Deserialize)]
struct BlameCase {
    entries: Vec<BlameEntry>,
    content: String,
    gens: Vec<i64>,
}

#[test]
fn blame_matches_ts() {
    let cases: Vec<BlameCase> = serde_json::from_str(&load("blame_cases.json")).unwrap();
    for case in cases {
        let entries: Vec<BlameSourceEntry> = case
            .entries
            .into_iter()
            .map(|e| BlameSourceEntry {
                gen: e.gen,
                timestamp: e.timestamp,
                module: e.module,
                patch: e.patch,
                prompt_diff: String::new(),
            })
            .collect();
        let gens: Vec<i64> = blame_file(&entries, &case.content)
            .into_iter()
            .map(|b| b.gen)
            .collect();
        assert_eq!(gens, case.gens);
    }
}

#[derive(Deserialize)]
struct FPos {
    line: usize,
    character: usize,
}

#[derive(Deserialize)]
struct FSpan {
    start: FPos,
    end: FPos,
}

#[derive(Deserialize)]
struct FRegion {
    present: bool,
    span: Option<FSpan>,
}

#[derive(Deserialize)]
struct FModule {
    value: String,
    span: FSpan,
}

#[derive(Deserialize)]
struct FDep {
    value: String,
    source: String,
    span: FSpan,
}

#[derive(Deserialize)]
struct FRef {
    module: String,
    span: FSpan,
}

#[derive(Deserialize)]
struct ScannerCase {
    doc: String,
    frontmatter: FRegion,
    body: FRegion,
    #[serde(rename = "moduleValue")]
    module_value: Option<FModule>,
    deps: Vec<FDep>,
    refs: Vec<FRef>,
}

fn span_eq(actual: Span, expected: &FSpan) -> bool {
    actual.start.line == expected.start.line
        && actual.start.character == expected.start.character
        && actual.end.line == expected.end.line
        && actual.end.character == expected.end.character
}

fn region_eq(actual: &RegionSpan, expected: &FRegion) -> bool {
    if actual.present != expected.present {
        return false;
    }
    match (&actual.span, &expected.span) {
        (Some(a), Some(e)) => span_eq(*a, e),
        (None, None) => true,
        _ => false,
    }
}

#[test]
fn scanner_matches_ts() {
    let cases: Vec<ScannerCase> = serde_json::from_str(&load("scanner_cases.json")).unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let scan = scan_document(&case.doc);
        assert!(
            region_eq(&scan.frontmatter, &case.frontmatter),
            "frontmatter region"
        );
        assert!(region_eq(&scan.body, &case.body), "body region");

        match (&scan.module_value, &case.module_value) {
            (Some(a), Some(e)) => {
                assert_eq!(a.value, e.value);
                assert!(span_eq(a.span, &e.span), "module span");
            }
            (None, None) => {}
            _ => panic!("module value presence mismatch"),
        }

        assert_eq!(scan.deps.len(), case.deps.len(), "deps count");
        for (a, e) in scan.deps.iter().zip(&case.deps) {
            assert_eq!(a.value, e.value);
            let source = match a.source {
                DepSource::Deps => "deps",
                DepSource::Extends => "extends",
            };
            assert_eq!(source, e.source);
            assert!(span_eq(a.span, &e.span), "dep span for {}", a.value);
        }

        assert_eq!(scan.refs.len(), case.refs.len(), "refs count");
        for (a, e) in scan.refs.iter().zip(&case.refs) {
            assert_eq!(a.module, e.module);
            assert!(span_eq(a.span, &e.span), "ref span for {}", a.module);
        }
    }
}

#[derive(Deserialize)]
struct SchemaCase {
    schema: String,
    input: serde_json::Value,
    accepts: bool,
}

#[test]
fn schema_accept_reject_matches_ts() {
    let cases: Vec<SchemaCase> = serde_json::from_str(&load("schema_cases.json")).unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let accepts = match case.schema.as_str() {
            "attribution" => validate_attribution(case.input.clone()).is_ok(),
            "attribution_entries" => parse_attribution_entries(case.input.clone()).is_ok(),
            "ml" => validate_ml(case.input.clone()).is_ok(),
            "ir" => validate_ir(case.input.clone()).is_ok(),
            "lock" => parse_lock(&case.input.to_string()).is_ok(),
            "map" => parse_map(&case.input.to_string()).is_ok(),
            other => panic!("unknown schema in fixture: {other}"),
        };
        assert_eq!(
            accepts, case.accepts,
            "{} acceptance mismatch for {}",
            case.schema, case.input
        );
    }
}
