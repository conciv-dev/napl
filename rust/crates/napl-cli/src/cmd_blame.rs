//! `napl blame`: git-blame-style line history for a generated file.

use std::path::Path;

use napl_core::blame::{blame_file, first_prompt_diff_line, BlameLine};
use napl_core::schemas::{file_history, JournalEntry, JournalMode};

use crate::error::CliResult;
use crate::paths::{rel_to, resolve_paths};
use crate::state::read_journal;

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

fn mode_str(mode: JournalMode) -> &'static str {
    match mode {
        JournalMode::Full => "full",
        JournalMode::Incremental => "incremental",
    }
}

fn why_line(prompt_diff: &str) -> String {
    let first = first_prompt_diff_line(prompt_diff);
    if first.is_empty() {
        "initial generation".to_string()
    } else {
        first
    }
}

fn format_blame_row(entry: &BlameLine) -> String {
    format!(
        "gen #{}  {}  {}  {}",
        entry.gen, entry.timestamp, entry.module, entry.text
    )
}

fn blame_gen(entries: &[JournalEntry], gen: i64) -> i32 {
    let Some(entry) = entries.iter().find(|candidate| candidate.gen == gen) else {
        println!("napl blame: no journal entry for gen #{gen}");
        return 1;
    };
    println!(
        "gen #{}  {}  {} ({})  mode: {}",
        entry.gen,
        entry.timestamp,
        entry.module,
        entry.target,
        mode_str(entry.mode)
    );
    println!();
    println!("prompt edit:");
    if entry.prompt_diff.trim().is_empty() {
        println!("  initial generation");
    } else {
        let indented: Vec<String> = entry
            .prompt_diff
            .split('\n')
            .map(|line| format!("  {line}"))
            .collect();
        println!("{}", indented.join("\n"));
    }
    println!();
    println!("files touched:");
    if entry.files.is_empty() {
        println!("  (none)");
    } else {
        for file in &entry.files {
            println!("  {}", file.path);
        }
    }
    0
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
        return Ok(blame_gen(&entries, gen));
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
