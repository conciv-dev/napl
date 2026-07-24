//! `napl gen <target>`: run the coding agent, gate on tests, lock, and derive
//! IR + attribution + machine layer. The orchestration counterpart of `gen.ts`.

use std::collections::BTreeMap;
use std::path::Path;

use napl_core::body_lines::{number_lines, prompt_body_lines};
use napl_core::drift::format_gen_drift_report;
use napl_core::hash::content_hash;
use napl_core::incremental::{
    diff_body_lines, incremental_unlock_list, select_intersecting_entries,
};
use napl_core::parse_output::extract_yaml;
use napl_core::prompts::{
    build_agent_task, build_attribution_repair, build_attribution_user,
    build_change_required_retry, build_incremental_task, build_ir_derivation_user,
    build_ml_derivation_user, DepSummary, IncrementalTaskInput, ATTRIBUTION_SYSTEM,
    IR_DERIVATION_SYSTEM, ML_DERIVATION_SYSTEM, YAML_STRICTNESS,
};
use napl_core::schemas::{file_patch, next_gen_number, record_attribution, record_unattributed};
use napl_core::schemas::{
    parse_attribution_entries, parse_frontmatter, parse_ml_entries, validate_attribution,
    validate_ir, validate_ml, Attribution, AttributionInput, FileInput, Frontmatter, JournalEntry,
    JournalFile, JournalMode, Ml, NaplMap, SchemaError, UnattributedInput,
};
use napl_core::targets::{get_adapter, TargetAdapter};
use napl_core::yaml::{attribution_to_yaml, ir_to_yaml, ml_to_yaml};

use gen_attribution_check::assert_attribution_sane;
use gen_classify::{first_meaningful_line, is_source_file, split_body_lines};
use gen_mode::{can_incremental, full_mode_message, incremental_mode_message, FullModeReason};
use gen_prompt_diff::compute_prompt_diff;

use crate::clock::now;
use crate::driftdetect::detect_gen_drift;
use crate::error::{CliError, CliResult};
use crate::fsutil::{self, READONLY_MODE, WRITABLE_MODE};
use crate::paths::{find_prompt_files, rel_to, resolve_paths, NaplPaths};
use crate::process::{
    acquire_gen_lock, llm_complete, require_claude, require_engine, resolve_engine,
    run_coding_agent, run_command, AgentEngine,
};
use crate::snapshot::{
    diff_snapshots, make_filter, snapshot_contents, snapshot_hashes, SnapshotFilter,
};
use crate::state::{load_prompt_aliases, read_journal, read_lock, read_map, write_map};

const MAX_ATTEMPTS: usize = 3;
const MAX_ATTRIBUTION_FILES: usize = 24;
const MAX_FILE_LINES: usize = 500;

/// Arguments for the gen command.
pub struct GenArgs<'a> {
    /// The target language.
    pub target: &'a str,
    /// Regenerate even when the prompt is up to date.
    pub force: bool,
    /// Force a from-scratch (non-incremental) generation.
    pub full: bool,
    /// Scope to a single module.
    pub module: Option<&'a str>,
}

struct GenSummary {
    generated: Vec<String>,
    skipped: Vec<String>,
}

/// Run gen: acquire the lock, generate, and print the summary line.
pub fn run(root: &Path, args: &GenArgs) -> CliResult<i32> {
    let paths = resolve_paths(root);
    let lock = read_lock(&paths.lock_path)?;
    if matches!(lock.backend, napl_core::schemas::Backend::AnthropicApi) {
        return Err(CliError::new(
            "the anthropic-api backend is not yet supported in the Rust CLI — use the TS CLI or set backend to \"claude-cli\" in .napl/lock.json.",
        ));
    }
    require_claude()?;
    let model = lock.model.clone();
    let engine = resolve_engine(&napl_core::schemas::resolve_agent_config(&lock));
    require_engine(&engine)?;

    let genlock = acquire_gen_lock(&paths.gen_lock_path)?;
    let result = run_gen_locked(root, &paths, args, &model, &engine);
    genlock.release();
    let summary = result?;
    println!(
        "generated {}, skipped {}",
        summary.generated.len(),
        summary.skipped.len()
    );
    Ok(0)
}

fn to_posix(path: &str) -> String {
    path.replace(std::path::MAIN_SEPARATOR, "/")
}

struct Attributed {
    abs: String,
    rel_to_target: String,
}

struct NumberedFiles {
    text: String,
    labels: Vec<String>,
}

fn build_numbered_files(attributed: &[Attributed]) -> NumberedFiles {
    let mut blocks: Vec<String> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    let mut count = 0;
    for file in attributed {
        if count >= MAX_ATTRIBUTION_FILES {
            break;
        }
        if !is_source_file(&file.rel_to_target) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&file.abs) else {
            continue;
        };
        let lines: Vec<String> = split_body_lines(&content)
            .into_iter()
            .take(MAX_FILE_LINES)
            .collect();
        blocks.push(format!(
            "=== FILE: {} ===\n{}",
            file.rel_to_target,
            number_lines(&lines)
        ));
        labels.push(file.rel_to_target.clone());
        count += 1;
    }
    NumberedFiles {
        text: blocks.join("\n\n"),
        labels,
    }
}

fn load_prior_body(prompts_at_gen_dir: &Path, module: &str) -> Option<String> {
    std::fs::read_to_string(prompts_at_gen_dir.join(format!("{module}.md"))).ok()
}

fn load_prior_attribution(
    attribution_dir: &Path,
    module: &str,
    target: &str,
) -> Option<Attribution> {
    let raw = std::fs::read_to_string(attribution_dir.join(format!("{module}.yaml"))).ok()?;
    let value: serde_yaml::Value = serde_yaml::from_str(&raw).ok()?;
    let json = serde_json::to_value(value).ok()?;
    let attribution = validate_attribution(json).ok()?;
    if attribution.target == target {
        Some(attribution)
    } else {
        None
    }
}

fn write_prior_body(prompts_at_gen_dir: &Path, module: &str, body: &str) -> CliResult<()> {
    fsutil::write(&prompts_at_gen_dir.join(format!("{module}.md")), body)?;
    Ok(())
}

fn collect_summaries(
    root: &Path,
    prompt_files: &[std::path::PathBuf],
) -> BTreeMap<String, DepSummary> {
    let mut summaries = BTreeMap::new();
    for file in prompt_files {
        let Ok(raw) = std::fs::read_to_string(file) else {
            continue;
        };
        if let Ok(parsed) = parse_frontmatter(&raw) {
            summaries.insert(
                rel_to(root, file),
                DepSummary {
                    module: parsed.frontmatter.module,
                    summary: first_meaningful_line(&parsed.body),
                },
            );
        }
    }
    summaries
}

fn yaml_to_json(raw: &str) -> Result<serde_json::Value, SchemaError> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(raw).map_err(|e| SchemaError::Deserialize(e.to_string()))?;
    serde_json::to_value(value).map_err(|e| SchemaError::Deserialize(e.to_string()))
}

enum TaskPlan {
    Full { deps: Vec<DepSummary> },
    Incremental { input: IncrementalTaskInput },
}

struct TaskBuilder {
    mode: JournalMode,
    plan: TaskPlan,
    unlock: Vec<String>,
}

fn build_task(
    adapter: &TargetAdapter,
    frontmatter: &Frontmatter,
    body: &str,
    plan: &TaskPlan,
    failure: Option<&str>,
) -> String {
    match plan {
        TaskPlan::Full { deps } => build_agent_task(adapter, frontmatter, body, deps, failure),
        TaskPlan::Incremental { input } => {
            build_incremental_task(adapter, frontmatter, body, input, failure)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_task_builder(
    root: &Path,
    paths: &NaplPaths,
    args: &GenArgs,
    frontmatter: &Frontmatter,
    body: &str,
    deps: Vec<DepSummary>,
    map: &NaplMap,
) -> TaskBuilder {
    let module = &frontmatter.module;
    let target = args.target;
    let target_record = map.prompts.get(module).and_then(|r| r.targets.get(target));
    let owned_files: Vec<String> = target_record.map(|r| r.files.clone()).unwrap_or_default();
    let use_incremental = can_incremental(
        args.full,
        target_record.is_some(),
        target_record.map(|r| r.unattributed) == Some(Some(true)),
        target_record
            .and_then(|r| r.prompt_hash_at_gen.as_ref())
            .is_some(),
    );

    if use_incremental {
        let prior_body = load_prior_body(&paths.prompts_at_gen_dir, module);
        let prior_attribution = load_prior_attribution(&paths.attribution_dir, module, target);
        if let (Some(prior_body), Some(prior_attribution)) = (prior_body, prior_attribution) {
            let diff = diff_body_lines(&prior_body, body);
            let intersecting =
                select_intersecting_entries(&prior_attribution.entries, &diff.changed_old_lines);
            let target_dir = paths.src_dir.join(target);
            let target_rel_to_root = rel_to(root, &target_dir);
            let unlock = incremental_unlock_list(&owned_files, &intersecting, &target_rel_to_root);
            println!(
                "{}",
                incremental_mode_message(
                    diff.changed_old_lines.len() + diff.changed_new_lines.len(),
                    intersecting.len()
                )
            );
            let owned_rel: Vec<String> = owned_files
                .iter()
                .map(|f| {
                    let abs = root.join(f);
                    to_posix(&rel_to(&target_dir, &abs))
                })
                .collect();
            return TaskBuilder {
                mode: JournalMode::Incremental,
                plan: TaskPlan::Incremental {
                    input: IncrementalTaskInput {
                        module: module.clone(),
                        diff: diff.unified,
                        intersecting_entries: intersecting,
                        owned_files: owned_rel,
                    },
                },
                unlock,
            };
        }
        println!("{}", full_mode_message(FullModeReason::NoPriorOnDisk));
    } else if args.full {
        println!("{}", full_mode_message(FullModeReason::ForcedFull));
    } else {
        println!("{}", full_mode_message(FullModeReason::NoPriorGen));
    }
    TaskBuilder {
        mode: JournalMode::Full,
        plan: TaskPlan::Full { deps },
        unlock: owned_files,
    }
}

fn unlock_files(root: &Path, files: &[String]) {
    for file in files {
        let abs = root.join(file);
        if abs.exists() {
            let _ = fsutil::set_mode(&abs, WRITABLE_MODE);
        }
    }
}

fn lock_attributed(attributed: &[Attributed]) -> CliResult<()> {
    for file in attributed {
        fsutil::set_mode(Path::new(&file.abs), READONLY_MODE)?;
    }
    Ok(())
}

struct DepsGate {
    module: String,
    target: String,
    cargo_toml_path: std::path::PathBuf,
    declared: Vec<String>,
}

fn cargo_path_dep_crates(cargo_toml: &str) -> Vec<String> {
    let mut deps: Vec<String> = Vec::new();
    let mut in_dependencies = false;
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dependencies = trimmed == "[dependencies]";
            continue;
        }
        if !in_dependencies || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((name, value)) = trimmed.split_once('=') else {
            continue;
        };
        let value = value.trim_start();
        if value.starts_with('{') && value.contains("path") {
            let name = name.trim();
            if !name.is_empty() {
                deps.push(name.to_string());
            }
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

fn check_declared_deps(
    module: &str,
    target: &str,
    path_dep_crates: &[String],
    declared: &[String],
) -> Result<(), String> {
    for dep in path_dep_crates {
        if !declared.iter().any(|d| d == dep) {
            return Err(format!(
                "gen failed for module '{module}' ({target}): the generated Cargo.toml declares a path dependency on the sibling crate '{dep}', which is not declared in the prompt's `deps:` frontmatter — declare it in `deps:` or remove the dependency."
            ));
        }
    }
    Ok(())
}

fn deps_gate_error(gate: &DepsGate) -> Option<String> {
    let cargo_toml = std::fs::read_to_string(&gate.cargo_toml_path).unwrap_or_default();
    let path_deps = cargo_path_dep_crates(&cargo_toml);
    check_declared_deps(&gate.module, &gate.target, &path_deps, &gate.declared).err()
}

struct AttemptResult {
    ok: bool,
    output: String,
    gate_error: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn run_attempts(
    adapter: &TargetAdapter,
    target_dir: &Path,
    model: &str,
    frontmatter: &Frontmatter,
    body: &str,
    plan: &TaskPlan,
    engine: &AgentEngine,
    crate_note: Option<&str>,
    deps_gate: Option<&DepsGate>,
) -> CliResult<AttemptResult> {
    let mut failure: Option<String> = None;
    let mut output = String::new();
    let mut gate_error: Option<String> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        println!("  attempt {attempt}/{MAX_ATTEMPTS}: running coding agent");
        let mut task = build_task(adapter, frontmatter, body, plan, failure.as_deref());
        if let Some(note) = crate_note {
            task.push_str(note);
        }
        let run = run_coding_agent(engine, &task, target_dir, model, &adapter.agent_tools)?;
        output = run.output;
        let cmd = adapter.test_command(&target_dir.to_string_lossy());
        let result = run_command(&cmd.command, &cmd.args, target_dir);
        if result.code == 0 {
            if let Some(gate) = deps_gate {
                if let Some(err) = deps_gate_error(gate) {
                    println!(
                        "  attempt {attempt}/{MAX_ATTEMPTS}: undeclared sibling dependency in Cargo.toml"
                    );
                    gate_error = Some(err.clone());
                    failure = Some(err);
                    continue;
                }
            }
            println!("  attempt {attempt}/{MAX_ATTEMPTS}: tests passed");
            return Ok(AttemptResult {
                ok: true,
                output,
                gate_error: None,
            });
        }
        gate_error = None;
        failure = Some(result.output);
        println!("  attempt {attempt}/{MAX_ATTEMPTS}: tests failed");
    }
    Ok(AttemptResult {
        ok: false,
        output,
        gate_error,
    })
}

fn retry_for_change(
    adapter: &TargetAdapter,
    target_dir: &Path,
    model: &str,
    base_task: &str,
    engine: &AgentEngine,
) -> CliResult<(String, bool)> {
    println!(
        "  prompt changed but the agent made no source edits — retrying once with an explicit change-required instruction"
    );
    let run = run_coding_agent(
        engine,
        &build_change_required_retry(base_task),
        target_dir,
        model,
        &adapter.agent_tools,
    )?;
    let cmd = adapter.test_command(&target_dir.to_string_lossy());
    let result = run_command(&cmd.command, &cmd.args, target_dir);
    Ok((run.output, result.code == 0))
}

fn derive_ir(
    model: &str,
    ir_dir: &Path,
    module: &str,
    body: &str,
    numbered_files: &str,
) -> CliResult<()> {
    if numbered_files.trim().is_empty() {
        return Ok(());
    }
    let mut last_error = String::new();
    for attempt in 1..=2 {
        let system = if attempt == 1 {
            IR_DERIVATION_SYSTEM.to_string()
        } else {
            format!("{IR_DERIVATION_SYSTEM}{YAML_STRICTNESS}")
        };
        let user = build_ir_derivation_user(module, body, numbered_files);
        match llm_complete(model, &system, &user)
            .map_err(|e| e.0)
            .and_then(|resp| {
                let mut json = yaml_to_json(&extract_yaml(&resp)).map_err(|e| e.to_string())?;
                if !json.is_object() {
                    json = serde_json::json!({});
                }
                if let Some(obj) = json.as_object_mut() {
                    obj.insert(
                        "module".to_string(),
                        serde_json::Value::String(module.to_string()),
                    );
                }
                validate_ir(json).map_err(|e| e.to_string())
            }) {
            Ok(ir) => {
                let ir_path = ir_dir.join(format!("{module}.yaml"));
                fsutil::write(&ir_path, &ir_to_yaml(&ir))?;
                return Ok(());
            }
            Err(msg) => last_error = msg,
        }
    }
    println!(
        "  warn: IR derivation for '{module}' failed (best-effort, IR skipped, gen continues): {last_error}"
    );
    Ok(())
}

fn derive_attribution_gated(
    model: &str,
    module: &str,
    target: &str,
    numbered_body: &str,
    numbered_files: &NumberedFiles,
) -> Result<Attribution, String> {
    let mut last_error = String::new();
    let mut last_output = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let repair = if attempt == 1 {
            String::new()
        } else {
            build_attribution_repair(&last_output, &last_error)
        };
        let system = if attempt == 1 {
            ATTRIBUTION_SYSTEM.to_string()
        } else {
            format!("{ATTRIBUTION_SYSTEM}{YAML_STRICTNESS}")
        };
        let user = format!(
            "{}{repair}",
            build_attribution_user(module, numbered_body, &numbered_files.text)
        );
        match llm_complete(model, &system, &user) {
            Ok(response) => {
                last_output.clone_from(&response);
                let result = (|| -> Result<Attribution, String> {
                    let json = yaml_to_json(&extract_yaml(&response)).map_err(|e| e.to_string())?;
                    let entries = parse_attribution_entries(json).map_err(|e| e.to_string())?;
                    let attribution = Attribution {
                        module: module.to_string(),
                        target: target.to_string(),
                        entries,
                    };
                    assert_attribution_sane(&attribution, &numbered_files.labels)?;
                    Ok(attribution)
                })();
                match result {
                    Ok(attribution) => {
                        println!(
                            "  attribution attempt {attempt}/{MAX_ATTEMPTS}: {} mapping(s) valid",
                            attribution.entries.len()
                        );
                        return Ok(attribution);
                    }
                    Err(msg) => {
                        println!("  attribution attempt {attempt}/{MAX_ATTEMPTS} failed: {msg}");
                        last_error = msg;
                    }
                }
            }
            Err(err) => {
                let message = err.0;
                println!("  attribution attempt {attempt}/{MAX_ATTEMPTS} failed: {message}");
                last_error = message;
            }
        }
    }
    Err(format!(
        "attribution derivation failed for module '{module}' ({target}) after {MAX_ATTEMPTS} attempts; last validation error: {last_error}"
    ))
}

fn derive_ml(
    model: &str,
    module: &str,
    target: &str,
    numbered_body: &str,
    change_summary: &str,
    agent_output: &str,
) -> Result<Vec<napl_core::schemas::MlEntry>, String> {
    let mut last_error = String::new();
    for attempt in 1..=2 {
        let system = if attempt == 1 {
            ML_DERIVATION_SYSTEM.to_string()
        } else {
            format!("{ML_DERIVATION_SYSTEM}{YAML_STRICTNESS}")
        };
        let user = build_ml_derivation_user(module, numbered_body, change_summary, agent_output);
        let result = llm_complete(model, &system, &user)
            .map_err(|e| e.0)
            .and_then(|resp| {
                let json = yaml_to_json(&extract_yaml(&resp)).map_err(|e| e.to_string())?;
                let entries = parse_ml_entries(json).map_err(|e| e.to_string())?;
                validate_ml(serde_json::json!({
                    "module": module,
                    "target": target,
                    "entries": [],
                }))
                .map_err(|e| e.to_string())?;
                Ok(entries)
            });
        match result {
            Ok(entries) => {
                println!(
                    "  machine-layer attempt {attempt}/2: {} entr(ies) valid",
                    entries.len()
                );
                return Ok(entries);
            }
            Err(msg) => {
                println!("  machine-layer attempt {attempt}/2 failed: {msg}");
                last_error = msg;
            }
        }
    }
    Err(format!(
        "machine-layer derivation failed for module '{module}' ({target}) after 2 attempts: {last_error}"
    ))
}

fn try_derive_ml(
    model: &str,
    module: &str,
    target: &str,
    numbered_body: &str,
    change_summary: &str,
    agent_output: &str,
) -> (Vec<napl_core::schemas::MlEntry>, Option<String>) {
    match derive_ml(
        model,
        module,
        target,
        numbered_body,
        change_summary,
        agent_output,
    ) {
        Ok(entries) => (entries, None),
        Err(msg) => (Vec::new(), Some(msg)),
    }
}

fn write_ml(
    ml_dir: &Path,
    module: &str,
    target: &str,
    entries: &[napl_core::schemas::MlEntry],
    machine_ext: &str,
) -> CliResult<()> {
    std::fs::create_dir_all(ml_dir)?;
    let ml = Ml {
        module: module.to_string(),
        target: target.to_string(),
        entries: entries.to_vec(),
    };
    fsutil::write(
        &ml_dir.join(format!("{module}{machine_ext}")),
        &ml_to_yaml(&ml),
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn enforce_no_op(
    entries: &[napl_core::schemas::MlEntry],
    error: Option<&str>,
    ml_dir: &Path,
    module: &str,
    target: &str,
    map: &NaplMap,
    map_path: &Path,
    machine_ext: &str,
) -> CliResult<()> {
    let has_no_op = error.is_none()
        && entries
            .iter()
            .any(|entry| entry.kind == napl_core::schemas::MlKind::NoOp);
    if has_no_op {
        println!(
            "  no-op: prompt changed but the agent produced no edits; the machine layer recorded a no-op note (module stays clean, squiggle surfaces)"
        );
        return Ok(());
    }
    if !entries.is_empty() {
        write_ml(ml_dir, module, target, entries, machine_ext)?;
    }
    write_map(map_path, map)?;
    let reason = match error {
        Some(msg) => format!("the machine-layer derivation failed ({msg})"),
        None => "the machine layer produced no \"no-op\" entry explaining why nothing changed"
            .to_string(),
    };
    println!(
        "  FAILED no-op check for '{module}' ({target}); module left stale, promptHashAtGen not updated"
    );
    Err(CliError::new(format!(
        "gen failed for module '{module}' ({target}): the prompt changed but the coding agent made no source edits, and {reason}. The requested change was NOT applied and the module is left stale; refine the prompt and re-run 'napl gen {target} --module {module} --force'."
    )))
}

#[allow(clippy::too_many_lines)]
fn run_gen_locked(
    root: &Path,
    paths: &NaplPaths,
    args: &GenArgs,
    model: &str,
    engine: &AgentEngine,
) -> CliResult<GenSummary> {
    let target = args.target;
    let adapter = get_adapter(target).map_err(CliError::new)?;
    let target_dir = paths.src_dir.join(target);
    std::fs::create_dir_all(&target_dir)?;
    write_guard_files(&target_dir)?;
    refresh_workspace_manifest(&target_dir, &adapter, None)?;

    let filter: SnapshotFilter = make_filter(
        &adapter.attribution_exclude_dirs,
        &adapter.attribution_exclude_files,
        &adapter.attribution_exclude_root_files,
        &adapter.attribution_exclude_suffixes,
    );
    let mut map = read_map(&paths.map_path)?;
    let aliases = load_prompt_aliases(&paths.lock_path);
    let prompt_files = find_prompt_files(root, &aliases)?;
    crate::discovery::check_duplicate_modules(root, &prompt_files)?;
    let summaries = collect_summaries(root, &prompt_files);
    let (_module_crate, crate_deps) = crate_assignment(root, &prompt_files, target);
    let (mut existing_journal, journal_warnings) = read_journal(&paths.journal_path)?;
    for warning in &journal_warnings {
        println!("{warning}");
    }
    let heals = crate::healing::heal_moved_files(root, paths, &mut map, &existing_journal)?;
    if !heals.is_empty() {
        write_map(&paths.map_path, &map)?;
        existing_journal = read_journal(&paths.journal_path)?.0;
    }
    let mut next_gen = next_gen_number(&existing_journal);
    let mut journaled_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    for entry in &existing_journal {
        for file in &entry.files {
            journaled_paths.insert(file.path.clone());
        }
    }

    if !args.force {
        let prompt_paths = crate::discovery::module_paths(root, &prompt_files);
        let drifts =
            detect_gen_drift(root, target, &map, &existing_journal, args.module, &prompt_paths)?;
        if !drifts.is_empty() {
            println!("{}", format_gen_drift_report(&drifts, target));
            let count: usize = drifts.iter().map(|d| d.files.len()).sum();
            return Err(CliError::new(format!(
                "gen blocked: {count} generated file(s) across {} module(s) have drifted from their prompts for target '{target}'. Resolve the drift shown above, or pass --force to discard the edits and regenerate.",
                drifts.len()
            )));
        }
    }

    let mut generated: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for file in &prompt_files {
        let raw = std::fs::read_to_string(file)?;
        let rel = rel_to(root, file);
        let parsed = parse_frontmatter(&raw)?;
        let frontmatter = parsed.frontmatter;
        let body = parsed.body;
        if !frontmatter.targets.iter().any(|t| t == target) {
            continue;
        }
        let module = frontmatter.module.clone();
        if let Some(scope) = args.module {
            if module != scope {
                continue;
            }
        }

        let machine_ext =
            napl_core::extensions::machine_extension_for_prompt(&file.to_string_lossy());
        let prompt_hash = content_hash(&raw);
        if !napl_core::schemas::is_prompt_gen_stale(
            map.prompts.get(&module),
            target,
            &prompt_hash,
            args.force,
        ) {
            skipped.push(module.clone());
            println!("skip    {module} ({target}) up to date");
            continue;
        }

        let deps: Vec<DepSummary> = summaries
            .iter()
            .filter(|(other_rel, _)| *other_rel != &rel)
            .map(|(_, summary)| summary.clone())
            .collect();

        println!("gen     {module} ({target})");
        let declared_crate = crate::discovery::declared_crate(&raw);
        let member_dir = declared_crate.clone().unwrap_or_else(|| module.clone());
        if let Some(crate_name) = &declared_crate {
            let empty = std::collections::BTreeSet::new();
            let path_deps: Vec<String> = crate_deps
                .get(crate_name)
                .unwrap_or(&empty)
                .iter()
                .cloned()
                .collect();
            refresh_declared_crate(&target_dir, crate_name, &module, &path_deps)?;
            println!(
                "  crate: {module} grouped into member crate '{crate_name}' as src/{module}.rs (Cargo.toml + lib.rs toolchain-owned)"
            );
        }
        refresh_workspace_manifest(&target_dir, &adapter, Some(&member_dir))?;
        let builder = build_task_builder(root, paths, args, &frontmatter, &body, deps, &map);

        unlock_files(root, &builder.unlock);

        let prior_hash_at_gen = map
            .prompts
            .get(&module)
            .and_then(|r| r.targets.get(target))
            .and_then(|t| t.prompt_hash_at_gen.clone());
        let prompt_changed = prior_hash_at_gen
            .as_ref()
            .is_some_and(|h| h != &prompt_hash);

        let crate_note = declared_crate.as_ref().map(|crate_name| {
            format!(
                "\n\nCRATE GROUPING: this module groups into the shared member crate `{crate_name}`. Write your implementation ONLY to `{crate_name}/src/{module}.rs`. The crate's `{crate_name}/Cargo.toml` and `{crate_name}/src/lib.rs` are written and owned by the toolchain — do not create, edit, or delete them."
            )
        });

        let deps_gate = if adapter.workspace_layout && declared_crate.is_none() {
            let declared: Vec<String> = crate_deps
                .get(&member_dir)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default();
            Some(DepsGate {
                module: module.clone(),
                target: target.to_string(),
                cargo_toml_path: target_dir.join(&member_dir).join("Cargo.toml"),
                declared,
            })
        } else {
            None
        };

        let before = snapshot_hashes(&target_dir, &filter)?;
        let prior_contents = snapshot_contents(&target_dir, &filter)?;
        let attempt = run_attempts(
            &adapter,
            &target_dir,
            model,
            &frontmatter,
            &body,
            &builder.plan,
            engine,
            crate_note.as_deref(),
            deps_gate.as_ref(),
        )?;
        if !attempt.ok {
            if let Some(message) = attempt.gate_error {
                return Err(CliError::new(message));
            }
            return Err(CliError::new(format!(
                "code generation failed for module '{module}' ({target}) after {MAX_ATTEMPTS} attempts."
            )));
        }
        let mut agent_output = attempt.output;

        let mut after = snapshot_hashes(&target_dir, &filter)?;
        let mut changed = diff_snapshots(&before, &after);

        if prompt_changed && changed.is_empty() {
            let mut base_task = build_task(&adapter, &frontmatter, &body, &builder.plan, None);
            if let Some(note) = &crate_note {
                base_task.push_str(note);
            }
            let (output, tests_passed) =
                retry_for_change(&adapter, &target_dir, model, &base_task, engine)?;
            agent_output = output;
            after = snapshot_hashes(&target_dir, &filter)?;
            changed = diff_snapshots(&before, &after);
            if !changed.is_empty() && !tests_passed {
                return Err(CliError::new(format!(
                    "code generation failed for module '{module}' ({target}): the change-required retry produced edits but its tests did not pass."
                )));
            }
        }

        let no_op_case = prompt_changed && changed.is_empty();

        let journal_files = build_journal_files(
            root,
            &changed,
            &before,
            &after,
            &prior_contents,
            &journaled_paths,
        );
        let prompt_diff = compute_prompt_diff(
            load_prior_body(&paths.prompts_at_gen_dir, &module).as_deref(),
            &body,
        );

        // Assemble attributed set: changed files plus surviving prior files.
        let mut attributed_rel: Vec<String> = changed
            .iter()
            .map(|abs| rel_to(root, Path::new(abs)))
            .collect();
        if let Some(prior_files) = map.prompts.get(&module).and_then(|r| r.targets.get(target)) {
            for prior in &prior_files.files {
                if root.join(prior).exists() && !attributed_rel.contains(prior) {
                    attributed_rel.push(prior.clone());
                }
            }
        }
        attributed_rel.sort();
        attributed_rel.dedup();

        let mut attributed: Vec<Attributed> = Vec::new();
        let mut files: Vec<FileInput> = Vec::new();
        for rel_to_root in &attributed_rel {
            let abs = root.join(rel_to_root);
            if !abs.exists() {
                continue;
            }
            let abs_str = abs.to_string_lossy().into_owned();
            let hash = after.get(&abs_str).cloned().unwrap_or_else(|| {
                content_hash(&std::fs::read_to_string(&abs).unwrap_or_default())
            });
            attributed.push(Attributed {
                abs: abs_str,
                rel_to_target: to_posix(&rel_to(&target_dir, &abs)),
            });
            files.push(FileInput {
                file_path: rel_to_root.clone(),
                hash,
            });
        }

        let numbered_files = build_numbered_files(&attributed);
        let numbered_body = number_lines(&prompt_body_lines(&raw).lines);
        derive_ir(model, &paths.ir_dir, &module, &body, &numbered_files.text)?;

        let change_summary = if changed.is_empty() || numbered_files.text.trim().is_empty() {
            "NO CHANGES".to_string()
        } else {
            numbered_files.text.clone()
        };

        let declared_targets = frontmatter.targets.clone();

        if numbered_files.text.trim().is_empty() {
            let (entries, error) = try_derive_ml(
                model,
                &module,
                target,
                &numbered_body,
                &change_summary,
                &agent_output,
            );
            if no_op_case {
                enforce_no_op(
                    &entries,
                    error.as_deref(),
                    &paths.ml_dir,
                    &module,
                    target,
                    &map,
                    &paths.map_path,
                    machine_ext,
                )?;
            }
            lock_attributed(&attributed)?;
            record_attribution(
                &mut map,
                &AttributionInput {
                    rel: module.clone(),
                    module: module.clone(),
                    prompt_hash: prompt_hash.clone(),
                    target: target.to_string(),
                    declared_targets: declared_targets.clone(),
                    files: files.clone(),
                },
            );
            write_prior_body(&paths.prompts_at_gen_dir, &module, &body)?;
            write_ml(&paths.ml_dir, &module, target, &entries, machine_ext)?;
            println!("  attributed {} file(s) to {module}", files.len());
            println!("  attribution: no source files to map; span attribution skipped");
            println!(
                "  machine layer: {} entr(ies) -> {}",
                entries.len(),
                rel_to(root, &paths.ml_dir.join(format!("{module}{machine_ext}")))
            );
            record_journal(
                paths,
                &mut next_gen,
                &module,
                target,
                &prompt_hash,
                &prompt_diff,
                builder.mode,
                &journal_files,
                &mut journaled_paths,
                root,
            )?;
            generated.push(module.clone());
            continue;
        }

        let attribution = match derive_attribution_gated(
            model,
            &module,
            target,
            &numbered_body,
            &numbered_files,
        ) {
            Ok(attribution) => attribution,
            Err(cause) => {
                record_unattributed(
                    &mut map,
                    &UnattributedInput {
                        rel: module.clone(),
                        module: module.clone(),
                        prompt_hash: prompt_hash.clone(),
                        target: target.to_string(),
                        declared_targets: declared_targets.clone(),
                        files: files.iter().map(|f| f.file_path.clone()).collect(),
                    },
                );
                write_map(&paths.map_path, &map)?;
                println!(
                    "  FAILED attribution for '{module}' ({target}); files left unlocked, target marked unattributed"
                );
                return Err(CliError::new(format!(
                    "gen failed for module '{module}' ({target}): required prompt attribution could not be derived after {MAX_ATTEMPTS} attempts. The generated files were left unlocked and the target is marked unattributed; re-run 'napl gen {target} --force' after resolving the issue. {cause}"
                )));
            }
        };

        let (ml_entries, ml_error) = try_derive_ml(
            model,
            &module,
            target,
            &numbered_body,
            &change_summary,
            &agent_output,
        );
        if no_op_case {
            enforce_no_op(
                &ml_entries,
                ml_error.as_deref(),
                &paths.ml_dir,
                &module,
                target,
                &map,
                &paths.map_path,
                machine_ext,
            )?;
        }

        lock_attributed(&attributed)?;
        record_attribution(
            &mut map,
            &AttributionInput {
                rel: module.clone(),
                module: module.clone(),
                prompt_hash: prompt_hash.clone(),
                target: target.to_string(),
                declared_targets: declared_targets.clone(),
                files: files.clone(),
            },
        );
        write_prior_body(&paths.prompts_at_gen_dir, &module, &body)?;
        let out_path = paths.attribution_dir.join(format!("{module}.yaml"));
        std::fs::create_dir_all(&paths.attribution_dir)?;
        fsutil::write(&out_path, &attribution_to_yaml(&attribution))?;
        write_ml(&paths.ml_dir, &module, target, &ml_entries, machine_ext)?;
        println!("  attributed {} file(s) to {module}", files.len());
        println!(
            "  attribution: {} mapping(s) -> {}",
            attribution.entries.len(),
            rel_to(root, &out_path)
        );
        match &ml_error {
            Some(msg) => println!(
                "  warn: machine-layer derivation failed (non-fatal, empty {machine_ext} written): {msg}"
            ),
            None => println!(
                "  machine layer: {} entr(ies) -> {}",
                ml_entries.len(),
                rel_to(root, &paths.ml_dir.join(format!("{module}{machine_ext}")))
            ),
        }
        record_journal(
            paths,
            &mut next_gen,
            &module,
            target,
            &prompt_hash,
            &prompt_diff,
            builder.mode,
            &journal_files,
            &mut journaled_paths,
            root,
        )?;
        generated.push(module.clone());
    }

    write_map(&paths.map_path, &map)?;
    Ok(GenSummary { generated, skipped })
}

#[allow(clippy::too_many_arguments)]
fn record_journal(
    paths: &NaplPaths,
    next_gen: &mut i64,
    module: &str,
    target: &str,
    prompt_hash: &str,
    prompt_diff: &str,
    mode: JournalMode,
    journal_files: &[JournalFile],
    journaled_paths: &mut std::collections::HashSet<String>,
    root: &Path,
) -> CliResult<()> {
    let entry = JournalEntry {
        gen: *next_gen,
        timestamp: now(),
        module: module.to_string(),
        target: target.to_string(),
        prompt_hash: prompt_hash.to_string(),
        prompt_diff: prompt_diff.to_string(),
        mode,
        files: journal_files.to_vec(),
    };
    crate::state::append_journal_entry(&paths.journal_path, &entry)?;
    for file in journal_files {
        journaled_paths.insert(file.path.clone());
    }
    println!(
        "  journal: gen #{} recorded ({} file patch(es)) -> {}",
        *next_gen,
        journal_files.len(),
        rel_to(root, &paths.journal_path)
    );
    *next_gen += 1;
    Ok(())
}

fn build_journal_files(
    root: &Path,
    changed: &[String],
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
    prior_contents: &BTreeMap<String, String>,
    journaled_paths: &std::collections::HashSet<String>,
) -> Vec<JournalFile> {
    let mut files = Vec::new();
    for abs in changed {
        let Ok(new_content) = std::fs::read_to_string(abs) else {
            continue;
        };
        let rel_path = to_posix(&rel_to(root, Path::new(abs)));
        let prior_for_patch = if journaled_paths.contains(&rel_path) {
            prior_contents.get(abs).map(String::as_str)
        } else {
            None
        };
        files.push(JournalFile {
            path: rel_path,
            patch: file_patch(prior_for_patch, &new_content),
            hash_before: before.get(abs).cloned(),
            hash_after: after
                .get(abs)
                .cloned()
                .unwrap_or_else(|| content_hash(&new_content)),
        });
    }
    files
}

fn write_guard_files(target_dir: &Path) -> CliResult<()> {
    std::fs::create_dir_all(target_dir)?;
    for name in napl_core::guard::GUARD_FILE_NAMES {
        fsutil::write(&target_dir.join(name), napl_core::guard::GUARD_DOC)?;
    }
    Ok(())
}

/// The immediate subdirectories of `target_dir` that are member crates (contain
/// a `Cargo.toml`), sorted.
fn member_crate_dirs(target_dir: &Path) -> Vec<String> {
    let mut members: Vec<String> = Vec::new();
    let Ok(entries) = std::fs::read_dir(target_dir) else {
        return members;
    };
    for entry in entries.flatten() {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
            && entry.path().join("Cargo.toml").is_file()
        {
            members.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    members.sort();
    members
}

/// Refresh the toolchain-owned workspace root manifest for a Cargo-workspace
/// target. Members are every existing member crate directory plus (when a gen is
/// about to run) the current module's directory, so the in-gen `cargo test` gate
/// at the workspace root covers every module and no member crate is ever an
/// orphan. A no-op for single-package targets.
fn refresh_workspace_manifest(
    target_dir: &Path,
    adapter: &TargetAdapter,
    current_module: Option<&str>,
) -> CliResult<()> {
    if !adapter.workspace_layout {
        return Ok(());
    }
    let mut members = member_crate_dirs(target_dir);
    if let Some(module) = current_module {
        if !members.iter().any(|m| m == module) {
            members.push(module.to_string());
        }
    }
    members.sort();
    members.dedup();
    fsutil::write(
        &target_dir.join("Cargo.toml"),
        &napl_core::targets::workspace_manifest_toml(&members),
    )?;
    Ok(())
}

/// Write the toolchain-owned files of a declared member crate (named by a
/// module's frontmatter `crate:` key). Like the workspace root manifest, the
/// crate's `Cargo.toml` and `src/lib.rs` are written and owned by the toolchain,
/// refreshed each gen, excluded from attribution, and never locked — module
/// ownership stays at the file level (`src/<module>.rs`). `lib.rs` exposes every
/// module source file present plus the current module (so the in-gen `cargo test`
/// compiles it); `Cargo.toml` carries the crate's merged sibling path-deps.
fn refresh_declared_crate(
    target_dir: &Path,
    crate_name: &str,
    current_module: &str,
    path_deps: &[String],
) -> CliResult<()> {
    use std::fmt::Write as _;
    let crate_dir = target_dir.join(crate_name);
    let src_dir = crate_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    let mut modules: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&src_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name.strip_suffix(".rs") {
                if stem != "lib" {
                    modules.push(stem.to_string());
                }
            }
        }
    }
    if !modules.iter().any(|m| m == current_module) {
        modules.push(current_module.to_string());
    }
    modules.sort();
    modules.dedup();

    let mut lib = String::new();
    for module in &modules {
        let _ = writeln!(lib, "pub mod {module};");
    }
    fsutil::write(&src_dir.join("lib.rs"), &lib)?;

    let mut manifest = String::new();
    manifest.push_str("[package]\n");
    let _ = writeln!(manifest, "name = \"{crate_name}\"");
    manifest.push_str("version = \"0.1.0\"\n");
    manifest.push_str("edition = \"2021\"\n");
    manifest.push_str("\n[dependencies]\n");
    let mut deps = path_deps.to_vec();
    deps.sort();
    deps.dedup();
    for dep in &deps {
        let _ = writeln!(manifest, "{dep} = {{ path = \"../{dep}\" }}");
    }
    fsutil::write(&crate_dir.join("Cargo.toml"), &manifest)?;
    Ok(())
}

/// The `module -> declared crate` assignment and each declared crate's merged
/// sibling path-deps, read from every prompt frontmatter targeting `target`. A
/// module with no `crate:` key maps to a crate named after itself (the default
/// per-module layout).
fn crate_assignment(
    root: &Path,
    prompt_files: &[std::path::PathBuf],
    target: &str,
) -> (
    BTreeMap<String, String>,
    BTreeMap<String, std::collections::BTreeSet<String>>,
) {
    let mut module_crate: BTreeMap<String, String> = BTreeMap::new();
    let mut deps_by_module: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in prompt_files {
        let Ok(raw) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(parsed) = parse_frontmatter(&raw) else {
            continue;
        };
        if !parsed.frontmatter.targets.iter().any(|t| t == target) {
            continue;
        }
        let module = parsed.frontmatter.module.clone();
        let crate_name =
            crate::discovery::declared_crate(&raw).unwrap_or_else(|| module.clone());
        module_crate.insert(module.clone(), crate_name);
        deps_by_module.insert(module, parsed.frontmatter.deps.clone());
    }
    let _ = root;
    let mut crate_deps: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();
    for (module, crate_name) in &module_crate {
        let entry = crate_deps.entry(crate_name.clone()).or_default();
        for dep in deps_by_module.get(module).into_iter().flatten() {
            let dep_crate = module_crate.get(dep).cloned().unwrap_or_else(|| dep.clone());
            if &dep_crate != crate_name {
                entry.insert(dep_crate);
            }
        }
    }
    (module_crate, crate_deps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use napl_core::incremental::diff_body_lines;
    use napl_core::schemas::{AttributionEntry, LineRange};

    fn entry(file: &str) -> AttributionEntry {
        AttributionEntry {
            prompt_lines: LineRange::new(1, 1),
            file: file.to_string(),
            lines: LineRange::new(1, 1),
            note: String::new(),
        }
    }

    // --- gen_classify: is_source_file ---

    #[test]
    fn is_source_file_accepts_known_extensions_and_rejects_config_and_others() {
        assert!(is_source_file("src/lib.rs"));
        assert!(is_source_file("app.tsx"));
        assert!(is_source_file("styles.css"));
        assert!(is_source_file("dir/x.jsx"));
        assert!(is_source_file("page.html"));
        assert!(!is_source_file("vite.config.ts"));
        assert!(!is_source_file("tailwind.config.js"));
        assert!(!is_source_file("README.md"));
        assert!(!is_source_file("noext"));
    }

    // --- gen_classify: first_meaningful_line ---

    #[test]
    fn first_meaningful_line_strips_headings_and_caps_length() {
        assert_eq!(first_meaningful_line("# Title\n\nBody line"), "Title");
        assert_eq!(first_meaningful_line("\n\n  hello world  \n"), "hello world");
        assert_eq!(first_meaningful_line("### Deep\nmore"), "Deep");
        assert_eq!(first_meaningful_line(""), "(no description)");
        assert_eq!(first_meaningful_line("   \n\t\n"), "(no description)");
        let long = "x".repeat(200);
        assert_eq!(first_meaningful_line(&long).chars().count(), 120);
    }

    // --- gen_classify: split_body_lines ---

    #[test]
    fn split_body_lines_splits_and_strips_cr() {
        assert_eq!(split_body_lines("a\r\nb\nc"), vec!["a", "b", "c"]);
        assert_eq!(split_body_lines(""), vec![""]);
        assert_eq!(split_body_lines("x\r\n"), vec!["x", ""]);
    }

    // --- gen_prompt_diff: compute_prompt_diff ---

    #[test]
    fn compute_prompt_diff_empty_when_no_prior_or_unchanged() {
        assert_eq!(compute_prompt_diff(None, "body"), "");
        assert_eq!(compute_prompt_diff(Some("body"), "body"), "");
    }

    #[test]
    fn compute_prompt_diff_uses_body_line_diff_when_changed() {
        assert_eq!(
            compute_prompt_diff(Some("old line"), "new line"),
            diff_body_lines("old line", "new line").unified
        );
    }

    // --- gen_attribution_check: assert_attribution_sane ---

    fn attribution(entries: Vec<AttributionEntry>) -> Attribution {
        Attribution {
            module: "m".to_string(),
            target: "rust".to_string(),
            entries,
        }
    }

    #[test]
    fn assert_attribution_sane_ok_when_no_files_and_no_entries() {
        assert_eq!(assert_attribution_sane(&attribution(vec![]), &[]), Ok(()));
    }

    #[test]
    fn assert_attribution_sane_rejects_empty_entries_with_attributed_files() {
        assert_eq!(
            assert_attribution_sane(&attribution(vec![]), &["a.ts".to_string()]),
            Err("attribution has no entries but the module has attributed source files".to_string())
        );
    }

    #[test]
    fn assert_attribution_sane_ok_when_entry_in_allowed_set() {
        assert_eq!(
            assert_attribution_sane(&attribution(vec![entry("a.ts")]), &["a.ts".to_string()]),
            Ok(())
        );
    }

    #[test]
    fn assert_attribution_sane_rejects_entry_outside_allowed_set() {
        assert_eq!(
            assert_attribution_sane(&attribution(vec![entry("b.ts")]), &["a.ts".to_string()]),
            Err("attribution entry references file \"b.ts\" which is outside the attributed file set".to_string())
        );
    }

    // --- gen_mode: can_incremental + message renderers ---

    #[test]
    fn can_incremental_requires_record_hash_not_full_not_unattributed() {
        assert!(can_incremental(false, true, false, true));
        assert!(!can_incremental(true, true, false, true));
        assert!(!can_incremental(false, false, false, true));
        assert!(!can_incremental(false, true, true, true));
        assert!(!can_incremental(false, true, false, false));
    }

    #[test]
    fn cargo_path_dep_crates_extracts_only_path_deps_in_dependencies() {
        let manifest = "[package]\nname = \"blame\"\n\n[dependencies]\ntext_diff = { path = \"../text_diff\" }\nserde = { version = \"1\", features = [\"derive\"] }\nsha2 = \"0.10\"\n\n[dev-dependencies]\nhelper = { path = \"../helper\" }\n";
        assert_eq!(
            cargo_path_dep_crates(manifest),
            vec!["text_diff".to_string()]
        );
    }

    #[test]
    fn check_declared_deps_accepts_declared_and_rejects_undeclared() {
        assert_eq!(
            check_declared_deps("blame", "rust", &["text_diff".to_string()], &["text_diff".to_string()]),
            Ok(())
        );
        assert_eq!(
            check_declared_deps("adder", "rust", &["helper".to_string()], &[]),
            Err("gen failed for module 'adder' (rust): the generated Cargo.toml declares a path dependency on the sibling crate 'helper', which is not declared in the prompt's `deps:` frontmatter — declare it in `deps:` or remove the dependency.".to_string())
        );
    }

    #[test]
    fn mode_messages_render_exact_strings() {
        assert_eq!(
            incremental_mode_message(3, 2),
            "  mode: INCREMENTAL — 3 changed prompt line(s), 2 owned region(s) affected"
        );
        assert_eq!(
            full_mode_message(FullModeReason::NoPriorOnDisk),
            "  mode: full (no prior prompt body or attribution on disk to diff against)"
        );
        assert_eq!(
            full_mode_message(FullModeReason::ForcedFull),
            "  mode: full (forced --full)"
        );
        assert_eq!(
            full_mode_message(FullModeReason::NoPriorGen),
            "  mode: full (no prior successful gen for this target)"
        );
    }
}
