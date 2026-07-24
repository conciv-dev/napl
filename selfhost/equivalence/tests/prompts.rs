//! Equivalence gate for the `prompts` module.
//!
//! This is the EXACT unit-test corpus of the hand-written `napl-core` `prompts`
//! module (rust/crates/napl-core/src/prompts.rs), replayed against the
//! NAPL-generated `prompts` crate under selfhost/.napl/src/rust/prompts/. The
//! seven corpus cases are replayed verbatim; the derivation-system constants and
//! the adapter-independent builders are additionally pinned BYTE-EXACT, because
//! the conformance corpus asserts substrings of these strings and the fake
//! backend routes on the system-prompt markers — so stage1 swap-in requires them
//! to reproduce byte-for-byte.
//!
//! Inputs are built from the NAPL-generated sibling crates (`targets` adapters,
//! `schemas_frontmatter::Frontmatter`, `schemas_attribution::AttributionEntry`),
//! proving the composition `prompts -> {targets, schemas_frontmatter,
//! schemas_attribution, schemas_line_range}`.

use prompts::{
    build_agent_task, build_attribution_repair, build_attribution_user,
    build_change_required_retry, build_incremental_task, build_ir_derivation_user,
    build_ml_derivation_user, build_reconcile_task, IncrementalTaskInput, ReconcileFile,
    ATTRIBUTION_SYSTEM, IR_DERIVATION_SYSTEM, ML_DERIVATION_SYSTEM, YAML_STRICTNESS,
};
use schemas_attribution::AttributionEntry;
use schemas_frontmatter::Frontmatter;
use schemas_line_range::LineRange;
use targets::get_adapter;

fn fm() -> Frontmatter {
    Frontmatter {
        module: "greeting".to_string(),
        deps: Vec::new(),
        targets: vec!["typescript".to_string()],
        tests: Vec::new(),
        crate_name: None,
    }
}

// ---- the seven hand-written corpus cases ----

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
fn reconcile_task_carries_prompt_and_diff() {
    let task = build_reconcile_task(
        "greeting",
        "examples/greeting.napl",
        "Greet a person by name.",
        &[ReconcileFile {
            file: ".napl/src/typescript/greet.ts".to_string(),
            diff: "@@ -1,1 +1,1 @@\n-Hello\n+HELLO".to_string(),
        }],
    );
    assert!(task.contains("reconciling hand edits"));
    assert!(task.contains("Prompt file to amend: examples/greeting.napl"));
    assert!(task.contains("Greet a person by name."));
    assert!(task.contains("=== FILE: .napl/src/typescript/greet.ts ==="));
    assert!(task.contains("+HELLO"));
    assert!(task.contains("NEVER touch the generated source"));
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

// ---- byte-exact pins beyond the substring corpus (stage1 swap-in safety) ----

#[test]
fn derivation_system_constants_are_byte_exact() {
    assert_eq!(
        IR_DERIVATION_SYSTEM,
        "You derive a CONTRACT-LEVEL intermediate representation (IR) from finished source code.\nYou are given a prompt (the contract in prose) and the source files that implement it.\nProduce a YAML document capturing CONTRACTS, not implementation, with these keys:\n- module: the exact module name provided.\n- deps: list of dependency module names (may be empty).\n- types: exported/public types as structural, language-neutral entries, each with a\n  \"name\" and a \"description\".\n- functions: the public functions/components, each with a \"name\", a language-neutral\n  \"signature\" string, and a \"behavior\" string covering pre/postconditions.\n- tests: the behavioral test cases as data, each with \"name\", \"given\", \"expect\".\nDo NOT include control flow, concurrency, memory idioms, or syntax trees.\nOutput ONLY a single fenced ```yaml code block and nothing else."
    );
    assert_eq!(
        ATTRIBUTION_SYSTEM,
        "You map lines of a prompt (the contract, in prose) to the exact lines of generated source\ncode that implement them. You are given the prompt body with 1-based line numbers, and each\nimplementing source file with 1-based line numbers.\n\nProduce a YAML list. Each item is a mapping with these keys:\n- promptLines: [start, end]  — 1-based inclusive line range in the prompt body.\n- file: the source file path exactly as labelled (relative to the target src directory).\n- lines: [start, end]  — 1-based inclusive line range in that file.\n- note: a short phrase describing what this code does (e.g. \"trims whitespace\").\n\nA prompt line may map to multiple code ranges, and one code range may map to multiple\nprompt lines — emit one item per concrete mapping. Only map lines that carry real intent;\nskip blank lines, headings, and boilerplate. Keep ranges tight.\nOutput ONLY a single fenced ```yaml code block containing the list, and nothing else."
    );
    assert_eq!(
        ML_DERIVATION_SYSTEM,
        "You are the MACHINE LAYER of a prompt compiler. A prompt (the contract, in prose) was compiled\nto source code by a coding agent. Your job is to record — for the human who wrote the prompt —\nwhere the prompt was ambiguous, what you had to assume, and anything else worth surfacing about\nthis compile.\n\nYou are given: the numbered prompt body, the numbered changes the agent made to the source\n(or the literal text \"NO CHANGES\"), and the coding agent's final message.\n\nOutput a YAML list (which may be empty). Each item has these keys:\n- promptLines: [start, end]  — 1-based inclusive line range in the PROMPT BODY the entry is about.\n- kind: one of \"ambiguity\", \"assumption\", \"note\", or \"no-op\".\n- message: a one-line human-facing summary.\n- reasoning: a fuller explanation of what was unclear and what was done instead.\n- suggestion: OPTIONAL — a proposed clearer rewording of the prompt line(s).\n\nHow to choose kind:\n- \"ambiguity\": the prompt line is vague, contradictory, self-referential, out of place, or reads\n  like an accidental or nonsensical insertion — an odd literal string, a phrase that does not fit\n  the surrounding requirement, or wording a careful engineer would stop and question. BE AGGRESSIVE\n  about flagging strange, surprising, or unmotivated phrasing; do NOT smooth it over or assume the\n  author must have meant something reasonable.\n- \"assumption\": you had to decide something the prompt left open; record the choice you made.\n- \"note\": something worth surfacing that is neither an ambiguity nor an assumption.\n- \"no-op\": use ONLY when the changes are \"NO CHANGES\". Explain why nothing was produced — the\n  requirement was already satisfied by existing code, or the instruction could not be understood or\n  acted upon. When the changes are \"NO CHANGES\" you MUST emit at least one \"no-op\" entry.\n\nAn empty list is valid ONLY when the prompt is entirely clear AND real changes were made.\nOutput ONLY a single fenced ```yaml code block containing the list, and nothing else."
    );
    assert_eq!(
        YAML_STRICTNESS,
        "\n\nEmit STRICTLY valid YAML. Quote every string value with double quotes and escape any inner double quotes, especially values containing a colon, quote, or bracket."
    );
}

#[test]
fn reconcile_task_is_byte_exact() {
    let task = build_reconcile_task(
        "greeting",
        "examples/greeting.napl",
        "Greet a person by name.",
        &[ReconcileFile {
            file: ".napl/src/typescript/greet.ts".to_string(),
            diff: "@@ -1,1 +1,1 @@\n-Hello\n+HELLO".to_string(),
        }],
    );
    let expected = [
        "You are reconciling hand edits back into the prompt (the durable source of truth) for the module \"greeting\".",
        "A developer edited generated source files directly, so they no longer match the prompt.",
        "Amend the prompt file below so that regenerating from it would reproduce the edited behavior.",
        "Edit ONLY the prompt file(s) named here — NEVER touch the generated source under .napl/src.",
        "",
        "Prompt file to amend: examples/greeting.napl",
        "",
        "Current prompt body:",
        "\"\"\"",
        "Greet a person by name.",
        "\"\"\"",
        "",
        "Observed source edits (recorded baseline -> current), per file:",
        "=== FILE: .napl/src/typescript/greet.ts ===",
        "```diff",
        "@@ -1,1 +1,1 @@\n-Hello\n+HELLO",
        "```",
        "",
        "Requirements:",
        "- Rewrite or extend the prompt prose so its described behavior matches the edited source.",
        "- Keep the YAML frontmatter valid; change only what the behavior change requires.",
        "- Do not restate the diff verbatim — describe the intended behavior in the prompt's own voice.",
        "- Do not edit, create, or delete any file under .napl/src — those are regenerated from the prompt.",
    ]
    .join("\n");
    assert_eq!(task, expected);
}

#[test]
fn change_required_retry_is_byte_exact() {
    let expected = [
        "BASE",
        "",
        "CRITICAL: A previous attempt made NO source changes at all, yet the prompt REQUIRES a change.",
        "Either implement the change now with real edits to the source files, or — only if you are certain",
        "no code change is needed — state CLEARLY in your final message WHY no change is required (for",
        "example: the requirement is already satisfied by the existing code, or the instruction cannot be",
        "acted on because it is unclear or nonsensical). Do not finish silently without doing one of these.",
    ]
    .join("\n");
    assert_eq!(build_change_required_retry("BASE"), expected);
}

#[test]
fn ir_derivation_user_is_byte_exact() {
    let user = build_ir_derivation_user("greeting", "Greet a person by name.", "FILES");
    let expected = [
        "Module name: greeting",
        "",
        "Prompt (the contract, in prose):",
        "\"\"\"",
        "Greet a person by name.",
        "\"\"\"",
        "",
        "Implementing source files:",
        "FILES",
    ]
    .join("\n");
    assert_eq!(user, expected);
}

#[test]
fn attribution_user_is_byte_exact() {
    let user = build_attribution_user("greeting", "1: body", "FILES");
    let expected = [
        "Module: greeting",
        "",
        "Prompt body (1-based line numbers):",
        "1: body",
        "",
        "Implementing source files (1-based line numbers):",
        "FILES",
    ]
    .join("\n");
    assert_eq!(user, expected);
}

#[test]
fn attribution_repair_is_byte_exact() {
    let repair = build_attribution_repair("prev", "boom");
    let expected = [
        "",
        "Your PREVIOUS response was REJECTED as invalid and MUST be corrected. Return ONLY a single",
        "fenced ```yaml code block containing the list — no prose, no commentary, nothing else. Every",
        "item must reference a \"file\" that appears EXACTLY as labelled above, and the list must not be",
        "empty. Emit STRICTLY valid YAML with double-quoted string values.",
        "",
        "Your previous invalid response was:",
        "\"\"\"",
        "prev",
        "\"\"\"",
        "",
        "The validation error was:",
        "boom",
    ]
    .join("\n");
    assert_eq!(repair, expected);
}

#[test]
fn ml_derivation_user_is_byte_exact() {
    let user = build_ml_derivation_user("greeting", "1: body", "NO CHANGES", "");
    let expected = [
        "Module: greeting",
        "",
        "Prompt body (1-based line numbers) — the durable source of truth:",
        "1: body",
        "",
        "What the coding agent changed in the source (1-based line numbers), or \"NO CHANGES\":",
        "NO CHANGES",
        "",
        "The coding agent's final message:",
        "\"\"\"",
        "(no output captured)",
        "\"\"\"",
    ]
    .join("\n");
    assert_eq!(user, expected);
}
