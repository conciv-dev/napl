//! Equivalence gate for the `cmd_blame` module's pure rendering slice.
//!
//! Replays the EXACT unit-test corpus of the hand-written `napl-cli` `cmd_blame`
//! module (rust/crates/napl-cli/src/cmd_blame.rs — the `tests` module) against
//! the NAPL-generated `blame_render` crate, which composes on the generated
//! `blame` and `schemas_journal` crates by path.

use blame::BlameLine;
use blame_render::{format_blame_row, mode_str, render_blame_gen, why_line};
use schemas_journal::{JournalEntry, JournalFile, JournalMode};

fn entry(gen: i64, module: &str, prompt_diff: &str, files: Vec<JournalFile>) -> JournalEntry {
    JournalEntry {
        gen,
        timestamp: "2026-07-24T00:00:00.000Z".to_string(),
        module: module.to_string(),
        target: "typescript".to_string(),
        prompt_hash: "hp".to_string(),
        prompt_diff: prompt_diff.to_string(),
        mode: JournalMode::Full,
        files,
    }
}

fn journal_file(path: &str) -> JournalFile {
    JournalFile {
        path: path.to_string(),
        patch: String::new(),
        hash_before: None,
        hash_after: "h".to_string(),
    }
}

#[test]
fn mode_str_maps_every_variant() {
    assert_eq!(mode_str(JournalMode::Full), "full");
    assert_eq!(mode_str(JournalMode::Incremental), "incremental");
    assert_eq!(mode_str(JournalMode::Reconcile), "reconcile");
    assert_eq!(mode_str(JournalMode::Move), "move");
}

#[test]
fn format_blame_row_is_two_space_separated() {
    let line = BlameLine {
        line: 4,
        gen: 7,
        timestamp: "2026-07-24T00:00:00.000Z".to_string(),
        module: "greeting".to_string(),
        text: "export function greet() {".to_string(),
    };
    assert_eq!(
        format_blame_row(&line),
        "gen #7  2026-07-24T00:00:00.000Z  greeting  export function greet() {"
    );
}

#[test]
fn why_line_is_initial_generation_for_empty_diff() {
    assert_eq!(why_line(""), "initial generation");
}

#[test]
fn why_line_reports_the_first_added_prompt_line() {
    let diff = "@@ -1,1 +1,2 @@\n context\n+the new behavior line\n";
    assert_eq!(why_line(diff), "the new behavior line");
}

#[test]
fn render_blame_gen_not_found_exits_one() {
    let render = render_blame_gen(&[entry(1, "greeting", "", vec![])], 9);
    assert_eq!(render.text, "napl blame: no journal entry for gen #9");
    assert_eq!(render.exit_code, 1);
}

#[test]
fn render_blame_gen_initial_generation_no_files() {
    let render = render_blame_gen(&[entry(1, "greeting", "", vec![])], 1);
    assert_eq!(
        render.text,
        "gen #1  2026-07-24T00:00:00.000Z  greeting (typescript)  mode: full\n\nprompt edit:\n  initial generation\n\nfiles touched:\n  (none)"
    );
    assert_eq!(render.exit_code, 0);
}

#[test]
fn render_blame_gen_indents_prompt_diff_and_lists_files() {
    let render = render_blame_gen(
        &[entry(
            2,
            "greeting",
            "@@ -1,1 +1,2 @@\n context\n+added behavior\n",
            vec![journal_file("src/greeting.ts")],
        )],
        2,
    );
    assert_eq!(
        render.text,
        "gen #2  2026-07-24T00:00:00.000Z  greeting (typescript)  mode: full\n\nprompt edit:\n  @@ -1,1 +1,2 @@\n   context\n  +added behavior\n  \n\nfiles touched:\n  src/greeting.ts"
    );
    assert_eq!(render.exit_code, 0);
}
