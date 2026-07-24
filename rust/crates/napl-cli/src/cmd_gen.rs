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
const SOURCE_EXTENSIONS: [&str; 7] = [".ts", ".tsx", ".js", ".jsx", ".css", ".html", ".rs"];

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

fn first_meaningful_line(body: &str) -> String {
    for line in body.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = line.trim_start_matches('#').trim();
        if !trimmed.is_empty() {
            return trimmed.chars().take(120).collect();
        }
    }
    "(no description)".to_string()
}

fn is_source_file(rel_to_target: &str) -> bool {
    let base = rel_to_target.rsplit('/').next().unwrap_or(rel_to_target);
    for suffix in [".config.ts", ".config.tsx", ".config.js", ".config.jsx"] {
        if base.ends_with(suffix) {
            return false;
        }
    }
    let Some(dot) = base.rfind('.') else {
        return false;
    };
    SOURCE_EXTENSIONS.contains(&&base[dot..])
}

fn split_body_lines(content: &str) -> Vec<String> {
    content
        .split('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s).to_string())
        .collect()
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
    rel: &str,
    frontmatter: &Frontmatter,
    body: &str,
    deps: Vec<DepSummary>,
    map: &NaplMap,
) -> TaskBuilder {
    let module = &frontmatter.module;
    let target = args.target;
    let target_record = map.prompts.get(rel).and_then(|r| r.targets.get(target));
    let owned_files: Vec<String> = target_record.map(|r| r.files.clone()).unwrap_or_default();
    let can_incremental = !args.full
        && target_record.is_some()
        && target_record.map(|r| r.unattributed) != Some(Some(true))
        && target_record
            .and_then(|r| r.prompt_hash_at_gen.as_ref())
            .is_some();

    if can_incremental {
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
                "  mode: INCREMENTAL — {} changed prompt line(s), {} owned region(s) affected",
                diff.changed_old_lines.len() + diff.changed_new_lines.len(),
                intersecting.len()
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
        println!("  mode: full (no prior prompt body or attribution on disk to diff against)");
    } else if args.full {
        println!("  mode: full (forced --full)");
    } else {
        println!("  mode: full (no prior successful gen for this target)");
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

struct AttemptResult {
    ok: bool,
    output: String,
}

fn run_attempts(
    adapter: &TargetAdapter,
    target_dir: &Path,
    model: &str,
    frontmatter: &Frontmatter,
    body: &str,
    plan: &TaskPlan,
    engine: &AgentEngine,
) -> CliResult<AttemptResult> {
    let mut failure: Option<String> = None;
    let mut output = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        println!("  attempt {attempt}/{MAX_ATTEMPTS}: running coding agent");
        let task = build_task(adapter, frontmatter, body, plan, failure.as_deref());
        let run = run_coding_agent(engine, &task, target_dir, model, &adapter.agent_tools)?;
        output = run.output;
        let cmd = adapter.test_command(&target_dir.to_string_lossy());
        let result = run_command(&cmd.command, &cmd.args, target_dir);
        if result.code == 0 {
            println!("  attempt {attempt}/{MAX_ATTEMPTS}: tests passed");
            return Ok(AttemptResult { ok: true, output });
        }
        failure = Some(result.output);
        println!("  attempt {attempt}/{MAX_ATTEMPTS}: tests failed");
    }
    Ok(AttemptResult { ok: false, output })
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

fn assert_attribution_sane(attribution: &Attribution, allowed: &[String]) -> Result<(), String> {
    if !allowed.is_empty() && attribution.entries.is_empty() {
        return Err(
            "attribution has no entries but the module has attributed source files".to_string(),
        );
    }
    for entry in &attribution.entries {
        if !allowed.contains(&entry.file) {
            return Err(format!(
                "attribution entry references file \"{}\" which is outside the attributed file set",
                entry.file
            ));
        }
    }
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

    let filter: SnapshotFilter = make_filter(
        &adapter.attribution_exclude_dirs,
        &adapter.attribution_exclude_files,
        &adapter.attribution_exclude_suffixes,
    );
    let mut map = read_map(&paths.map_path)?;
    let aliases = load_prompt_aliases(&paths.lock_path);
    let prompt_files = find_prompt_files(root, &aliases)?;
    let summaries = collect_summaries(root, &prompt_files);
    let (existing_journal, journal_warnings) = read_journal(&paths.journal_path)?;
    for warning in &journal_warnings {
        println!("{warning}");
    }
    let mut next_gen = next_gen_number(&existing_journal);
    let mut journaled_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    for entry in &existing_journal {
        for file in &entry.files {
            journaled_paths.insert(file.path.clone());
        }
    }

    if !args.force {
        let drifts = detect_gen_drift(root, target, &map, &existing_journal, args.module)?;
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
            map.prompts.get(&rel),
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
        let builder = build_task_builder(root, paths, args, &rel, &frontmatter, &body, deps, &map);

        unlock_files(root, &builder.unlock);

        let prior_hash_at_gen = map
            .prompts
            .get(&rel)
            .and_then(|r| r.targets.get(target))
            .and_then(|t| t.prompt_hash_at_gen.clone());
        let prompt_changed = prior_hash_at_gen
            .as_ref()
            .is_some_and(|h| h != &prompt_hash);

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
        )?;
        if !attempt.ok {
            return Err(CliError::new(format!(
                "code generation failed for module '{module}' ({target}) after {MAX_ATTEMPTS} attempts."
            )));
        }
        let mut agent_output = attempt.output;

        let mut after = snapshot_hashes(&target_dir, &filter)?;
        let mut changed = diff_snapshots(&before, &after);

        if prompt_changed && changed.is_empty() {
            let base_task = build_task(&adapter, &frontmatter, &body, &builder.plan, None);
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
        if let Some(prior_files) = map.prompts.get(&rel).and_then(|r| r.targets.get(target)) {
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
                    rel: rel.clone(),
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
                        rel: rel.clone(),
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
                rel: rel.clone(),
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

fn compute_prompt_diff(prior_body: Option<&str>, body: &str) -> String {
    match prior_body {
        Some(prior) if prior != body => diff_body_lines(prior, body).unified,
        _ => String::new(),
    }
}

fn write_guard_files(target_dir: &Path) -> CliResult<()> {
    std::fs::create_dir_all(target_dir)?;
    for name in napl_core::guard::GUARD_FILE_NAMES {
        fsutil::write(&target_dir.join(name), napl_core::guard::GUARD_DOC)?;
    }
    Ok(())
}
