//! `napl watch <target>`: toolchain-owned auto-compile. Filesystem events on the
//! prompt files are debounced; on settle, the same code path as `napl gen` runs,
//! scoped to the changed modules and serialized via the existing gen.lock.
//!
//! `--once` is the deterministic conformance seam: it runs a single gen pass over
//! whatever is currently pending (stale) and exits, so a scenario can pin the
//! full output without racing real filesystem events.
//!
//! Stage1: the pure ignore predicate (`is_ignored`) is the NAPL-generated
//! `watch_filter` crate, re-exported here; this shell keeps the event loop, the
//! debounce, and the gen dispatch. The unit corpus below rides along as the
//! regression net.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

use napl_core::extensions::is_prompt_file;
use napl_core::schemas::parse_frontmatter;

use crate::cmd_gen::{self, GenArgs};
use crate::error::{CliError, CliResult};
use crate::paths::{rel_to, resolve_paths};
use crate::state::load_prompt_aliases;

use watch_filter::is_ignored;

/// Arguments for the watch command.
pub struct WatchArgs<'a> {
    /// The target language.
    pub target: &'a str,
    /// Scope to a single module by name.
    pub module: Option<&'a str>,
    /// Debounce window in milliseconds.
    pub debounce: u64,
    /// Process the currently-pending changes once, then exit (deterministic).
    pub once: bool,
}

fn gen_once(root: &Path, args: &WatchArgs) -> CliResult<i32> {
    cmd_gen::run(
        root,
        &GenArgs {
            target: args.target,
            force: false,
            full: false,
            module: args.module,
        },
    )
}

fn module_of(path: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    parse_frontmatter(&raw).ok().map(|p| p.frontmatter.module)
}

/// Run watch.
pub fn run(root: &Path, args: &WatchArgs) -> CliResult<i32> {
    if args.once {
        println!(
            "watch   {} (once) — running gen for pending changes",
            args.target
        );
        return gen_once(root, args);
    }
    run_loop(root, args)
}

fn run_loop(root: &Path, args: &WatchArgs) -> CliResult<i32> {
    use notify::{RecursiveMode, Watcher};

    let aliases = load_prompt_aliases(&resolve_paths(root).lock_path);
    let (tx, rx) = channel::<notify::Event>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })
    .map_err(|e| CliError::new(format!("could not start the filesystem watcher: {e}")))?;
    watcher
        .watch(root, RecursiveMode::Recursive)
        .map_err(|e| CliError::new(format!("could not watch {}: {e}", root.to_string_lossy())))?;

    let running = Arc::new(AtomicBool::new(true));
    {
        let running = Arc::clone(&running);
        ctrlc::set_handler(move || running.store(false, Ordering::SeqCst))
            .map_err(|e| CliError::new(format!("could not install the Ctrl-C handler: {e}")))?;
    }

    println!(
        "watch   {} — watching prompt files (debounce {}ms). Press Ctrl-C to stop.",
        args.target, args.debounce
    );

    let poll = Duration::from_millis(200);
    let debounce = Duration::from_millis(args.debounce);
    while running.load(Ordering::SeqCst) {
        let event = match rx.recv_timeout(poll) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let mut changed: BTreeSet<PathBuf> = BTreeSet::new();
        collect_prompt_paths(&event, root, &aliases, &mut changed);
        while let Ok(next) = rx.recv_timeout(debounce) {
            collect_prompt_paths(&next, root, &aliases, &mut changed);
            if !running.load(Ordering::SeqCst) {
                break;
            }
        }
        if changed.is_empty() {
            continue;
        }
        run_for_changed(root, args, &changed);
    }

    println!("watch stopped.");
    Ok(0)
}

fn collect_prompt_paths(
    event: &notify::Event,
    root: &Path,
    aliases: &[String],
    out: &mut BTreeSet<PathBuf>,
) {
    for path in &event.paths {
        if is_ignored(path, root) {
            continue;
        }
        let name = path.to_string_lossy();
        if is_prompt_file(&name, Some(aliases)) {
            out.insert(path.clone());
        }
    }
}

fn run_for_changed(root: &Path, args: &WatchArgs, changed: &BTreeSet<PathBuf>) {
    let mut modules: BTreeSet<String> = BTreeSet::new();
    for path in changed {
        println!("change  {}", rel_to(root, path));
        if let Some(module) = module_of(path) {
            modules.insert(module);
        }
    }
    if let Some(scope) = args.module {
        modules.retain(|m| m == scope);
    }
    if modules.is_empty() {
        return;
    }
    for module in &modules {
        let result = cmd_gen::run(
            root,
            &GenArgs {
                target: args.target,
                force: false,
                full: false,
                module: Some(module),
            },
        );
        if let Err(error) = result {
            eprintln!("napl: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_paths_under_toolchain_and_vcs_dirs() {
        let root = Path::new("/proj");
        assert!(is_ignored(&root.join("node_modules/dep.js"), root));
        assert!(is_ignored(&root.join(".napl/src/rust/x.rs"), root));
        assert!(is_ignored(&root.join(".git/HEAD"), root));
        assert!(is_ignored(&root.join("src/a/.napl/b"), root));
    }

    #[test]
    fn keeps_ordinary_prompt_paths() {
        let root = Path::new("/proj");
        assert!(!is_ignored(&root.join("examples/greeting.napl"), root));
        assert!(!is_ignored(&root.join("src/lib.rs"), root));
    }
}
