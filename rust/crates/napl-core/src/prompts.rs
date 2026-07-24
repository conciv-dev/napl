//! Pure builders for the coding-agent task and the IR/attribution/machine-layer
//! LLM prompts. Ported line-for-line from the TypeScript `prompts.ts`; the exact
//! text is load-bearing (the conformance corpus asserts substrings of the agent
//! input, and the fake backend routes on system-prompt markers).

use crate::schemas::{AttributionEntry, Frontmatter};
use crate::targets::TargetAdapter;

/// A dependency module summary passed to the full-generation task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepSummary {
    /// The module name.
    pub module: String,
    /// A one-line summary of the module.
    pub summary: String,
}

/// Inputs describing an incremental (diff-scoped) generation.
#[derive(Debug, Clone)]
pub struct IncrementalTaskInput {
    /// The module name.
    pub module: String,
    /// The unified diff of the prompt body change.
    pub diff: String,
    /// The owned attribution regions intersecting the change.
    pub intersecting_entries: Vec<AttributionEntry>,
    /// The files owned by this module, relative to the target src dir.
    pub owned_files: Vec<String>,
}

fn last_chars(text: &str, count: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= count {
        return text.to_string();
    }
    chars[chars.len() - count..].iter().collect()
}

/// Build the incremental coding-agent task, mirroring `buildIncrementalTask`.
#[must_use]
pub fn build_incremental_task(
    adapter: &TargetAdapter,
    frontmatter: &Frontmatter,
    body: &str,
    input: &IncrementalTaskInput,
    failure: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!(
        "You are a coding agent making an INCREMENTAL update to the existing module \"{}\"",
        frontmatter.module
    ));
    parts.extend(
        [
            "in the current working directory. Its source code already exists and passes tests. The prompt",
            "(the durable source of truth) has CHANGED. Make the MINIMAL code edits needed to realize the",
            "prompt change — do not restructure or rewrite untouched code, do not reformat unrelated files.",
            "",
            "Target idiom guidance:",
        ]
        .iter()
        .map(ToString::to_string),
    );
    parts.push(adapter.idiom_guidance.clone());
    parts.extend(
        [
            "",
            "Unified diff of the prompt change (old vs new prompt body):",
            "```diff",
        ]
        .iter()
        .map(ToString::to_string),
    );
    parts.push(input.diff.trim().to_string());
    parts.push("```".to_string());
    if !input.intersecting_entries.is_empty() {
        parts.extend(
            [
                "",
                "The changed prompt lines are currently implemented by these owned code regions.",
                "Prefer editing exactly these regions:",
            ]
            .iter()
            .map(ToString::to_string),
        );
        for entry in &input.intersecting_entries {
            let note = if entry.note.is_empty() {
                String::new()
            } else {
                format!(" — {}", entry.note)
            };
            parts.push(format!(
                "- {} lines {}-{}{note}",
                entry.file, entry.lines.start, entry.lines.end
            ));
        }
    }
    if !input.owned_files.is_empty() {
        parts.push(String::new());
        parts.push("Files owned by this module (edit only what the change requires):".to_string());
        for file in &input.owned_files {
            parts.push(format!("- {file}"));
        }
    }
    parts.extend(
        [
            "",
            "Full current prompt body (the target behavior after the change):",
            "\"\"\"",
        ]
        .iter()
        .map(ToString::to_string),
    );
    parts.push(body.trim().to_string());
    parts.extend(
        ["\"\"\"", "", "Requirements:"]
            .iter()
            .map(ToString::to_string),
    );
    parts.push(format!(
        "- Ensure \"{}\" passes from this directory before you finish.",
        adapter.test_command_label
    ));
    parts.extend(
        [
            "- Update the existing tests to match the new behavior; do not delete unrelated tests.",
            "- Use only functions/hooks, named exports, and no dead code.",
            "- Do not edit files under node_modules; use the package manager for dependencies.",
        ]
        .iter()
        .map(ToString::to_string),
    );
    if let Some(failure) = failure {
        parts.extend(
            [
                "",
                "A previous attempt FAILED its tests. Read the current files, fix the code and/or tests,",
                "and make the suite pass. Test output from the failed attempt:",
                "```",
            ]
            .iter()
            .map(ToString::to_string),
        );
        parts.push(last_chars(failure.trim(), 6000));
        parts.push("```".to_string());
    }
    parts.join("\n")
}

/// Append the change-required escalation, mirroring `buildChangeRequiredRetry`.
#[must_use]
pub fn build_change_required_retry(base_task: &str) -> String {
    [
        base_task,
        "",
        "CRITICAL: A previous attempt made NO source changes at all, yet the prompt REQUIRES a change.",
        "Either implement the change now with real edits to the source files, or — only if you are certain",
        "no code change is needed — state CLEARLY in your final message WHY no change is required (for",
        "example: the requirement is already satisfied by the existing code, or the instruction cannot be",
        "acted on because it is unclear or nonsensical). Do not finish silently without doing one of these.",
    ]
    .join("\n")
}

/// Build the full-generation coding-agent task, mirroring `buildAgentTask`.
#[must_use]
pub fn build_agent_task(
    adapter: &TargetAdapter,
    frontmatter: &Frontmatter,
    body: &str,
    deps: &[DepSummary],
    failure: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!(
        "You are a coding agent implementing the module \"{}\" as real, runnable",
        frontmatter.module
    ));
    parts.extend(
        [
            "source code in the current working directory. Write and edit files directly; scaffold the",
            "project and install dependencies as needed. The prompt below is the durable source of truth.",
            "",
            "Target idiom guidance:",
        ]
        .iter()
        .map(ToString::to_string),
    );
    parts.push(adapter.idiom_guidance.clone());
    parts.push(String::new());
    parts.push(format!("Module: {}", frontmatter.module));
    if !frontmatter.deps.is_empty() {
        parts.push(format!(
            "Declared dependencies: {}",
            frontmatter.deps.join(", ")
        ));
    }
    if !deps.is_empty() {
        parts.push(String::new());
        parts.push(
            "Other modules in this project (for context, do not reimplement them):".to_string(),
        );
        for dep in deps {
            parts.push(format!("- {}: {}", dep.module, dep.summary));
        }
    }
    parts.extend(
        [
            "",
            "Specification to implement (implement exactly this behavior):",
            "\"\"\"",
        ]
        .iter()
        .map(ToString::to_string),
    );
    parts.push(body.trim().to_string());
    parts.extend(
        ["\"\"\"", "", "Requirements:"]
            .iter()
            .map(ToString::to_string),
    );
    parts.push(format!(
        "- Ensure \"{}\" passes from this directory before you finish.",
        adapter.test_command_label
    ));
    parts.extend(
        [
            "- Write a real test suite covering the described behavior.",
            "- Use only functions/hooks, named exports, and no dead code.",
            "- Do not edit files under node_modules; use the package manager for dependencies.",
        ]
        .iter()
        .map(ToString::to_string),
    );
    if let Some(failure) = failure {
        parts.extend(
            [
                "",
                "A previous attempt FAILED its tests. Read the current files, fix the code and/or tests,",
                "and make the suite pass. Test output from the failed attempt:",
                "```",
            ]
            .iter()
            .map(ToString::to_string),
        );
        parts.push(last_chars(failure.trim(), 6000));
        parts.push("```".to_string());
    }
    parts.join("\n")
}

/// System prompt for IR derivation (contains the `intermediate representation` marker).
pub const IR_DERIVATION_SYSTEM: &str = "You derive a CONTRACT-LEVEL intermediate representation (IR) from finished source code.\nYou are given a prompt (the contract in prose) and the source files that implement it.\nProduce a YAML document capturing CONTRACTS, not implementation, with these keys:\n- module: the exact module name provided.\n- deps: list of dependency module names (may be empty).\n- types: exported/public types as structural, language-neutral entries, each with a\n  \"name\" and a \"description\".\n- functions: the public functions/components, each with a \"name\", a language-neutral\n  \"signature\" string, and a \"behavior\" string covering pre/postconditions.\n- tests: the behavioral test cases as data, each with \"name\", \"given\", \"expect\".\nDo NOT include control flow, concurrency, memory idioms, or syntax trees.\nOutput ONLY a single fenced ```yaml code block and nothing else.";

/// Build the IR-derivation user prompt, mirroring `buildIrDerivationUser`.
#[must_use]
pub fn build_ir_derivation_user(module: &str, body: &str, files: &str) -> String {
    [
        format!("Module name: {module}"),
        String::new(),
        "Prompt (the contract, in prose):".to_string(),
        "\"\"\"".to_string(),
        body.trim().to_string(),
        "\"\"\"".to_string(),
        String::new(),
        "Implementing source files:".to_string(),
        files.to_string(),
    ]
    .join("\n")
}

/// System prompt for attribution derivation.
pub const ATTRIBUTION_SYSTEM: &str = "You map lines of a prompt (the contract, in prose) to the exact lines of generated source\ncode that implement them. You are given the prompt body with 1-based line numbers, and each\nimplementing source file with 1-based line numbers.\n\nProduce a YAML list. Each item is a mapping with these keys:\n- promptLines: [start, end]  — 1-based inclusive line range in the prompt body.\n- file: the source file path exactly as labelled (relative to the target src directory).\n- lines: [start, end]  — 1-based inclusive line range in that file.\n- note: a short phrase describing what this code does (e.g. \"trims whitespace\").\n\nA prompt line may map to multiple code ranges, and one code range may map to multiple\nprompt lines — emit one item per concrete mapping. Only map lines that carry real intent;\nskip blank lines, headings, and boilerplate. Keep ranges tight.\nOutput ONLY a single fenced ```yaml code block containing the list, and nothing else.";

/// Build the attribution user prompt, mirroring `buildAttributionUser`.
#[must_use]
pub fn build_attribution_user(module: &str, numbered_body: &str, files: &str) -> String {
    [
        format!("Module: {module}"),
        String::new(),
        "Prompt body (1-based line numbers):".to_string(),
        numbered_body.to_string(),
        String::new(),
        "Implementing source files (1-based line numbers):".to_string(),
        files.to_string(),
    ]
    .join("\n")
}

/// Build the attribution repair suffix, mirroring `buildAttributionRepair`.
#[must_use]
pub fn build_attribution_repair(previous_output: &str, error_message: &str) -> String {
    let mut previous: String = previous_output.trim().chars().take(4000).collect();
    previous = previous.trim().to_string();
    [
        "",
        "Your PREVIOUS response was REJECTED as invalid and MUST be corrected. Return ONLY a single",
        "fenced ```yaml code block containing the list — no prose, no commentary, nothing else. Every",
        "item must reference a \"file\" that appears EXACTLY as labelled above, and the list must not be",
        "empty. Emit STRICTLY valid YAML with double-quoted string values.",
        "",
        "Your previous invalid response was:",
        "\"\"\"",
        previous.as_str(),
        "\"\"\"",
        "",
        "The validation error was:",
        error_message,
    ]
    .join("\n")
}

/// System prompt for machine-layer derivation (contains the `MACHINE LAYER` marker).
pub const ML_DERIVATION_SYSTEM: &str = "You are the MACHINE LAYER of a prompt compiler. A prompt (the contract, in prose) was compiled\nto source code by a coding agent. Your job is to record — for the human who wrote the prompt —\nwhere the prompt was ambiguous, what you had to assume, and anything else worth surfacing about\nthis compile.\n\nYou are given: the numbered prompt body, the numbered changes the agent made to the source\n(or the literal text \"NO CHANGES\"), and the coding agent's final message.\n\nOutput a YAML list (which may be empty). Each item has these keys:\n- promptLines: [start, end]  — 1-based inclusive line range in the PROMPT BODY the entry is about.\n- kind: one of \"ambiguity\", \"assumption\", \"note\", or \"no-op\".\n- message: a one-line human-facing summary.\n- reasoning: a fuller explanation of what was unclear and what was done instead.\n- suggestion: OPTIONAL — a proposed clearer rewording of the prompt line(s).\n\nHow to choose kind:\n- \"ambiguity\": the prompt line is vague, contradictory, self-referential, out of place, or reads\n  like an accidental or nonsensical insertion — an odd literal string, a phrase that does not fit\n  the surrounding requirement, or wording a careful engineer would stop and question. BE AGGRESSIVE\n  about flagging strange, surprising, or unmotivated phrasing; do NOT smooth it over or assume the\n  author must have meant something reasonable.\n- \"assumption\": you had to decide something the prompt left open; record the choice you made.\n- \"note\": something worth surfacing that is neither an ambiguity nor an assumption.\n- \"no-op\": use ONLY when the changes are \"NO CHANGES\". Explain why nothing was produced — the\n  requirement was already satisfied by existing code, or the instruction could not be understood or\n  acted upon. When the changes are \"NO CHANGES\" you MUST emit at least one \"no-op\" entry.\n\nAn empty list is valid ONLY when the prompt is entirely clear AND real changes were made.\nOutput ONLY a single fenced ```yaml code block containing the list, and nothing else.";

/// Build the machine-layer user prompt, mirroring `buildMlDerivationUser`.
#[must_use]
pub fn build_ml_derivation_user(
    module: &str,
    numbered_body: &str,
    change_summary: &str,
    agent_output: &str,
) -> String {
    let output = last_chars(agent_output.trim(), 4000);
    [
        format!("Module: {module}"),
        String::new(),
        "Prompt body (1-based line numbers) — the durable source of truth:".to_string(),
        numbered_body.to_string(),
        String::new(),
        "What the coding agent changed in the source (1-based line numbers), or \"NO CHANGES\":"
            .to_string(),
        change_summary.to_string(),
        String::new(),
        "The coding agent's final message:".to_string(),
        "\"\"\"".to_string(),
        if output.is_empty() {
            "(no output captured)".to_string()
        } else {
            output
        },
        "\"\"\"".to_string(),
    ]
    .join("\n")
}

/// The YAML-strictness suffix appended to a system prompt on retry.
pub const YAML_STRICTNESS: &str = "\n\nEmit STRICTLY valid YAML. Quote every string value with double quotes and escape any inner double quotes, especially values containing a colon, quote, or bracket.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemas::{AttributionEntry, Frontmatter, LineRange};
    use crate::targets::get_adapter;

    fn fm() -> Frontmatter {
        Frontmatter {
            module: "greeting".to_string(),
            deps: Vec::new(),
            targets: vec!["typescript".to_string()],
            tests: Vec::new(),
        }
    }

    #[test]
    fn full_task_names_module_and_spec() {
        let task = build_agent_task(
            &get_adapter("typescript").unwrap(),
            &fm(),
            "Greet a person by name.",
            &[],
            None,
        );
        assert!(task.contains("implementing the module \"greeting\""));
        assert!(task.contains("Greet a person by name."));
        assert!(!task.contains("INCREMENTAL update"));
    }

    #[test]
    fn full_task_appends_failure_block() {
        let task = build_agent_task(
            &get_adapter("typescript").unwrap(),
            &fm(),
            "x",
            &[],
            Some("FAIL boom"),
        );
        assert!(task.contains("A previous attempt FAILED its tests."));
        assert!(task.contains("FAIL boom"));
    }

    #[test]
    fn incremental_task_carries_diff_and_region() {
        let input = IncrementalTaskInput {
            module: "greeting".to_string(),
            diff: "@@ -1,1 +1,1 @@\n-Greet a person by name.\n+Greet a person by name, loudly."
                .to_string(),
            intersecting_entries: vec![AttributionEntry {
                prompt_lines: LineRange::new(1, 1),
                file: "greet.ts".to_string(),
                lines: LineRange::new(1, 1),
                note: "builds greeting".to_string(),
            }],
            owned_files: vec!["greet.ts".to_string()],
        };
        let task = build_incremental_task(
            &get_adapter("typescript").unwrap(),
            &fm(),
            "Greet a person by name, loudly.",
            &input,
            None,
        );
        assert!(task.contains("INCREMENTAL update"));
        assert!(task.contains("-Greet a person by name."));
        assert!(task.contains("+Greet a person by name, loudly."));
        assert!(task.contains("greet.ts lines 1-1 — builds greeting"));
    }

    #[test]
    fn change_required_appends_critical() {
        let out = build_change_required_retry("BASE");
        assert!(out.starts_with("BASE"));
        assert!(out.contains("CRITICAL: A previous attempt made NO source changes"));
    }

    #[test]
    fn system_prompts_carry_routing_markers() {
        assert!(IR_DERIVATION_SYSTEM.contains("intermediate representation"));
        assert!(ML_DERIVATION_SYSTEM.contains("MACHINE LAYER"));
        assert!(!ATTRIBUTION_SYSTEM.contains("intermediate representation"));
        assert!(!ATTRIBUTION_SYSTEM.contains("MACHINE LAYER"));
    }

    #[test]
    fn ml_user_marks_no_changes() {
        let user = build_ml_derivation_user("greeting", "1: body", "NO CHANGES", "");
        assert!(user.contains("NO CHANGES"));
        assert!(user.contains("(no output captured)"));
    }
}
