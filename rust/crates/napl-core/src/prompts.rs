//! Stage1 adapter over the NAPL-generated `prompts` crate.

pub use gen_prompts::{
    build_agent_task, build_attribution_repair, build_attribution_user,
    build_change_required_retry, build_incremental_task, build_ir_derivation_user,
    build_ml_derivation_user, build_reconcile_task, DepSummary, IncrementalTaskInput, ReconcileFile,
    ATTRIBUTION_SYSTEM, IR_DERIVATION_SYSTEM, ML_DERIVATION_SYSTEM, YAML_STRICTNESS,
};

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
}
