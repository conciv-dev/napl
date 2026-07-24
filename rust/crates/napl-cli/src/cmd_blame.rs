//! `napl blame`: git-blame-style line history for a generated file.
//!
//! Stage1: the pure rendering slice (`mode_str`, `format_blame_row`, `why_line`,
//! `render_blame_gen`) is the NAPL-generated `blame_render` crate, re-exported
//! here; this shell keeps the journal read, the blame computation, and the
//! printing. The unit corpus below rides along as the regression net.

use std::path::Path;

use napl_core::blame::{blame_file, BlameLine};
use napl_core::schemas::file_history;

use crate::error::CliResult;
use crate::paths::{rel_to, resolve_paths};
use crate::state::read_journal;

use blame_render::{format_blame_row, render_blame_gen, why_line};

/// Options for the blame command.
pub struct BlameArgs<'a> {
    /// The generated file path (relative or absolute).
    pub file: Option<&'a str>,
    /// Restrict to a single 1-based line.
    pub line: Option<usize>,
    /// Print a single gen's summary instead.
    pub gen: Option<i64>,
    /// Also show the prompt edit that caused each line.
    pub verbose: bool,
}

fn normalize_file_arg(root: &Path, file: &str) -> String {
    let path = Path::new(file);
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    rel_to(root, &abs)
}

/// Run blame.
pub fn run(root: &Path, args: &BlameArgs) -> CliResult<i32> {
    let paths = resolve_paths(root);
    let (entries, warnings) = read_journal(&paths.journal_path)?;
    for warning in &warnings {
        println!("{warning}");
    }

    if entries.is_empty() {
        println!(
            "napl blame: no gen journal found — run `napl gen` to start recording line history."
        );
        return Ok(1);
    }

    if let Some(gen) = args.gen {
        let render = render_blame_gen(&entries, gen);
        println!("{}", render.text);
        return Ok(render.exit_code);
    }

    let Some(file) = args.file else {
        println!("napl blame: provide a generated file path, or --gen <n>.");
        return Ok(1);
    };

    let rel_path = normalize_file_arg(root, file);
    let history = file_history(&entries, &rel_path);
    if history.is_empty() {
        println!(
            "napl blame: no journal history for {rel_path} — is it a generated file under .napl/src/?"
        );
        return Ok(1);
    }

    let abs = root.join(&rel_path);
    if !abs.exists() {
        println!("napl blame: file not found on disk: {rel_path}");
        return Ok(1);
    }
    let content = std::fs::read_to_string(&abs)?;
    let blamed = blame_file(&history, &content);

    let rows: Vec<&BlameLine> = match args.line {
        Some(line) => blamed.iter().filter(|entry| entry.line == line).collect(),
        None => blamed.iter().collect(),
    };
    if let Some(line) = args.line {
        if rows.is_empty() {
            println!(
                "napl blame: line {line} is out of range for {rel_path} ({} line(s)).",
                blamed.len()
            );
            return Ok(1);
        }
    }

    let mut shown_why: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for entry in rows {
        println!("{}", format_blame_row(entry));
        if args.verbose && !shown_why.contains(&entry.gen) {
            shown_why.insert(entry.gen);
            let prompt_diff = history
                .iter()
                .find(|h| h.gen == entry.gen)
                .map_or("", |h| h.prompt_diff.as_str());
            println!("    why (gen #{}): {}", entry.gen, why_line(prompt_diff));
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use blame_render::mode_str;
    use napl_core::schemas::{JournalEntry, JournalFile, JournalMode};

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
}
