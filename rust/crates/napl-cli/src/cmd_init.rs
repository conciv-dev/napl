//! `napl init`: scaffold the `.napl` layout, lock, map, guard docs, example
//! prompt, `.claude/settings.json`, and the pre-commit hook.

use std::path::{Path, PathBuf};

use napl_core::guard::{
    claude_settings_snippet, merge_claude_settings, SettingsMergeAction, GUARD_DOC,
    GUARD_FILE_NAMES, PRE_COMMIT_HOOK, PRE_COMMIT_HOOK_LINE,
};
use napl_core::schemas::empty_map;
use napl_core::targets::list_targets;

use crate::error::CliResult;
use crate::fsutil::{self, EXEC_MODE};
use crate::paths::resolve_paths;
use crate::state::{default_lock, write_lock, write_map};

const EXAMPLE_PROMPT: &str = "---\nmodule: greeting\ndeps: []\ntargets: [typescript]\ntests:\n  - name: greets by name\n    given: { name: World }\n    expect: { message: \"Hello, World!\" }\n  - name: trims surrounding whitespace\n    given: { name: \"  Ada  \" }\n    expect: { message: \"Hello, Ada!\" }\n---\n# Greeting\n\nExpose a `greet` function that takes a person's name and returns a friendly\ngreeting message.\n\n- The greeting has the form `Hello, <name>!`.\n- Leading and trailing whitespace in the name is trimmed before use.\n- An empty or whitespace-only name is rejected with an error.\n";

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn write_if_absent(path: &Path, contents: &str) -> CliResult<()> {
    if fsutil::exists(path) {
        println!("exists  {}", display(path));
        return Ok(());
    }
    fsutil::write(path, contents)?;
    println!("create  {}", display(path));
    Ok(())
}

fn write_guard_docs(src_dir: &Path) -> CliResult<()> {
    for target in list_targets() {
        let target_dir = src_dir.join(target);
        std::fs::create_dir_all(&target_dir)?;
        for name in GUARD_FILE_NAMES {
            write_if_absent(&target_dir.join(name), GUARD_DOC)?;
        }
    }
    Ok(())
}

fn write_claude_deny_rules(root: &Path) -> CliResult<()> {
    let settings_path = root.join(".claude").join("settings.json");
    let existing = fsutil::read_opt(&settings_path)?;
    let merge = merge_claude_settings(existing.as_deref());
    match merge.action {
        SettingsMergeAction::Create => {
            if let Some(content) = merge.content {
                std::fs::create_dir_all(root.join(".claude"))?;
                fsutil::write(&settings_path, &content)?;
                println!(
                    "create  {} (deny Edit on .napl/src/**)",
                    display(&settings_path)
                );
            }
        }
        SettingsMergeAction::Update => {
            if let Some(content) = merge.content {
                fsutil::write(&settings_path, &content)?;
                println!(
                    "update  {} (added deny rule for .napl/src/**)",
                    display(&settings_path)
                );
            }
        }
        SettingsMergeAction::Unchanged => {
            println!(
                "exists  {} already denies edits to .napl/src/**",
                display(&settings_path)
            );
        }
        SettingsMergeAction::Manual => {
            println!(
                "note: {} exists but could not be safely merged — add this to it yourself:",
                display(&settings_path)
            );
            println!("{}", claude_settings_snippet());
        }
    }
    Ok(())
}

fn find_git_dir(root: &Path) -> Option<PathBuf> {
    let mut dir = root.to_path_buf();
    loop {
        let candidate = dir.join(".git");
        if candidate.is_dir() {
            return Some(candidate);
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent.to_path_buf(),
            _ => return None,
        }
    }
}

fn install_pre_commit_hook(root: &Path) -> CliResult<()> {
    let Some(git_dir) = find_git_dir(root) else {
        println!(
            "note: no .git directory found — skipping pre-commit hook install (run napl init again after git init)"
        );
        return Ok(());
    };
    let hooks_dir = git_dir.join("hooks");
    let hook_path = hooks_dir.join("pre-commit");
    if fsutil::exists(&hook_path) {
        println!(
            "exists  {} — leaving it untouched; add \"{PRE_COMMIT_HOOK_LINE}\" to it to gate commits on drift",
            display(&hook_path)
        );
        return Ok(());
    }
    std::fs::create_dir_all(&hooks_dir)?;
    fsutil::write(&hook_path, PRE_COMMIT_HOOK)?;
    fsutil::set_mode(&hook_path, EXEC_MODE)?;
    println!(
        "create  {} (runs napl status; blocks commits on drift)",
        display(&hook_path)
    );
    Ok(())
}

/// Run init.
pub fn run(root: &Path) -> CliResult<i32> {
    let paths = resolve_paths(root);

    std::fs::create_dir_all(&paths.ir_dir)?;
    std::fs::create_dir_all(&paths.src_dir)?;
    std::fs::create_dir_all(&paths.examples_dir)?;

    if fsutil::exists(&paths.lock_path) {
        println!("exists  {}", display(&paths.lock_path));
    } else {
        write_lock(&paths.lock_path, &default_lock())?;
        println!(
            "create  {} (model: {}, backend: claude-cli)",
            display(&paths.lock_path),
            napl_core::schemas::DEFAULT_MODEL
        );
    }

    if fsutil::exists(&paths.map_path) {
        println!("exists  {}", display(&paths.map_path));
    } else {
        write_map(&paths.map_path, &empty_map())?;
        println!("create  {}", display(&paths.map_path));
    }

    write_guard_docs(&paths.src_dir)?;
    write_if_absent(&paths.examples_dir.join("greeting.napl"), EXAMPLE_PROMPT)?;
    write_claude_deny_rules(root)?;
    install_pre_commit_hook(root)?;

    println!(
        "initialized. Next: edit a *.napl prompt, then run \"napl gen <target>\" (e.g. napl gen typescript)."
    );
    println!("gen runs a coding agent that writes the source directly, then derives the IR and attribution.");
    Ok(0)
}
