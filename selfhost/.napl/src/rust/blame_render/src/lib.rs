//! Rendering `napl blame` output.
//!
//! Pure rendering core for the CLI's blame command: given journal and blame
//! data the I/O shell has already read from disk, produces the exact text
//! the command prints. No filesystem access, no process spawning.

use blame::BlameLine;
use schemas_journal::{JournalEntry, JournalMode};

/// Map a journal mode to its lowercase label.
pub fn mode_str(mode: JournalMode) -> &'static str {
    match mode {
        JournalMode::Full => "full",
        JournalMode::Incremental => "incremental",
        JournalMode::Reconcile => "reconcile",
    }
}

/// Render one blame row.
pub fn format_blame_row(entry: &BlameLine) -> String {
    format!(
        "gen #{}  {}  {}  {}",
        entry.gen, entry.timestamp, entry.module, entry.text
    )
}

/// Explain why a line exists.
pub fn why_line(prompt_diff: &str) -> String {
    let line = blame::first_prompt_diff_line(prompt_diff);
    if line.is_empty() {
        "initial generation".to_string()
    } else {
        line
    }
}

/// The result of rendering a single-generation blame summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameGenRender {
    pub text: String,
    pub exit_code: i32,
}

/// Render the single-generation summary for `napl blame --gen <n>`.
pub fn render_blame_gen(entries: &[JournalEntry], gen: i64) -> BlameGenRender {
    let Some(entry) = entries.iter().find(|e| e.gen == gen) else {
        return BlameGenRender {
            text: format!("napl blame: no journal entry for gen #{gen}"),
            exit_code: 1,
        };
    };

    let mut lines: Vec<String> = Vec::new();

    lines.push(format!(
        "gen #{}  {}  {} ({})  mode: {}",
        entry.gen,
        entry.timestamp,
        entry.module,
        entry.target,
        mode_str(entry.mode)
    ));
    lines.push(String::new());
    lines.push("prompt edit:".to_string());

    if entry.prompt_diff.trim().is_empty() {
        lines.push("  initial generation".to_string());
    } else {
        for piece in entry.prompt_diff.split('\n') {
            lines.push(format!("  {piece}"));
        }
    }

    lines.push(String::new());
    lines.push("files touched:".to_string());

    if entry.files.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for file in &entry.files {
            lines.push(format!("  {}", file.path));
        }
    }

    BlameGenRender {
        text: lines.join("\n"),
        exit_code: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemas_journal::JournalFile;

    fn entry(
        gen: i64,
        prompt_diff: &str,
        mode: JournalMode,
        files: Vec<JournalFile>,
    ) -> JournalEntry {
        JournalEntry {
            gen,
            timestamp: "2026-07-24T00:00:00.000Z".to_string(),
            module: "greeting".to_string(),
            target: "typescript".to_string(),
            prompt_hash: "hash".to_string(),
            prompt_diff: prompt_diff.to_string(),
            mode,
            files,
        }
    }

    fn file(path: &str) -> JournalFile {
        JournalFile {
            path: path.to_string(),
            patch: String::new(),
            hash_before: None,
            hash_after: "hash".to_string(),
        }
    }

    #[test]
    fn mode_str_maps_all_variants() {
        assert_eq!(mode_str(JournalMode::Full), "full");
        assert_eq!(mode_str(JournalMode::Incremental), "incremental");
        assert_eq!(mode_str(JournalMode::Reconcile), "reconcile");
    }

    #[test]
    fn format_blame_row_renders_expected_line() {
        let line = BlameLine {
            line: 1,
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
    fn why_line_empty_diff_returns_initial_generation() {
        assert_eq!(why_line(""), "initial generation");
    }

    #[test]
    fn why_line_non_empty_diff_returns_first_prompt_diff_line() {
        let diff = "--- a\n+++ b\n@@ -1,2 +1,2 @@\n-old line\n+the new behavior line\n";
        assert_eq!(why_line(diff), "the new behavior line");
    }

    #[test]
    fn render_blame_gen_no_matching_entry() {
        let entries = vec![entry(1, "", JournalMode::Full, vec![])];
        let result = render_blame_gen(&entries, 5);
        assert_eq!(result.exit_code, 1);
        assert_eq!(result.text, "napl blame: no journal entry for gen #5");
    }

    #[test]
    fn render_blame_gen_empty_diff_no_files() {
        let entries = vec![entry(1, "", JournalMode::Full, vec![])];
        let result = render_blame_gen(&entries, 1);
        assert_eq!(result.exit_code, 0);
        assert_eq!(
            result.text,
            "gen #1  2026-07-24T00:00:00.000Z  greeting (typescript)  mode: full\n\nprompt edit:\n  initial generation\n\nfiles touched:\n  (none)"
        );
    }

    #[test]
    fn render_blame_gen_whitespace_only_diff_treated_as_empty() {
        let entries = vec![entry(1, "   \n  ", JournalMode::Full, vec![])];
        let result = render_blame_gen(&entries, 1);
        assert!(result.text.contains("prompt edit:\n  initial generation"));
    }

    #[test]
    fn render_blame_gen_multiline_diff_and_files() {
        let diff = "line one\nline two";
        let entries = vec![entry(
            2,
            diff,
            JournalMode::Incremental,
            vec![file("src/a.ts"), file("src/b.ts")],
        )];
        let result = render_blame_gen(&entries, 2);
        assert_eq!(result.exit_code, 0);
        assert_eq!(
            result.text,
            "gen #2  2026-07-24T00:00:00.000Z  greeting (typescript)  mode: incremental\n\nprompt edit:\n  line one\n  line two\n\nfiles touched:\n  src/a.ts\n  src/b.ts"
        );
    }

    #[test]
    fn render_blame_gen_diff_with_trailing_newline_yields_final_blank_prefixed_line() {
        let diff = "line one\n";
        let entries = vec![entry(3, diff, JournalMode::Reconcile, vec![])];
        let result = render_blame_gen(&entries, 3);
        assert!(result.text.contains("prompt edit:\n  line one\n  \n\nfiles touched:"));
    }

    #[test]
    fn render_blame_gen_picks_first_matching_gen() {
        let entries = vec![
            entry(1, "", JournalMode::Full, vec![]),
            entry(2, "", JournalMode::Full, vec![]),
        ];
        let result = render_blame_gen(&entries, 2);
        assert_eq!(result.exit_code, 0);
        assert!(result.text.starts_with("gen #2"));
    }
}
