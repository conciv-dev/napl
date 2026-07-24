//! The `napl` binary: clap parsing and command dispatch. All user-facing errors
//! are printed to stderr as `napl: {message}` with exit code 1, matching the
//! TypeScript CLI's behavior.

mod clock;
mod cmd_blame;
mod cmd_build;
mod cmd_gen;
mod cmd_init;
mod cmd_reconcile;
mod cmd_status;
mod cmd_test;
mod cmd_watch;
mod driftdetect;
mod error;
mod fsutil;
mod paths;
mod process;
mod snapshot;
mod state;
mod statusclass;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "napl",
    disable_version_flag = true,
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create the .napl/ structure, lock.json, and an example prompt.
    Init,
    /// Deprecated — gen now works directly from prompts.
    Build,
    /// Run a coding agent that writes target code from prompts, then lock and derive IR + attribution.
    Gen {
        /// Target language (e.g. typescript, react).
        target: String,
        /// Regenerate every prompt even when the prompt has not changed.
        #[arg(short = 'f', long)]
        force: bool,
        /// Force from-scratch generation instead of automatic incremental mode.
        #[arg(long)]
        full: bool,
        /// Scope the run to a single module by name.
        #[arg(short = 'm', long)]
        module: Option<String>,
    },
    /// Git-blame-style line history for a generated file.
    Blame {
        /// A generated file under .napl/src/<target>/.
        file: Option<String>,
        /// Blame only a single 1-based line.
        #[arg(short = 'l', long)]
        line: Option<usize>,
        /// Print the summary of a single gen journal entry.
        #[arg(short = 'g', long)]
        gen: Option<i64>,
        /// Also show the prompt edit that caused each line.
        #[arg(short = 'v', long)]
        verbose: bool,
    },
    /// Fold drifted src edits back into the prompt, then leave the module stale for regen.
    Reconcile {
        /// Target language (e.g. typescript, react).
        target: String,
        /// Scope the run to a single module by name.
        #[arg(short = 'm', long)]
        module: Option<String>,
    },
    /// Watch prompt files and auto-run gen for changed modules on save.
    Watch {
        /// Target language (e.g. typescript, react).
        target: String,
        /// Scope the run to a single module by name.
        #[arg(short = 'm', long)]
        module: Option<String>,
        /// Debounce window in milliseconds before a settled change triggers gen.
        #[arg(long, default_value_t = 400)]
        debounce: u64,
        /// Process the currently-pending changes once, then exit.
        #[arg(long)]
        once: bool,
    },
    /// Report clean / prompt-stale / DRIFT / unattributed per prompt.
    Status,
    /// Run generated tests for a target without regenerating.
    Test {
        /// Target language.
        #[arg(default_value = "typescript")]
        target: String,
    },
    /// Start the language server over stdio (used by editor extensions).
    Lsp,
}

fn main() {
    std::process::exit(real_main());
}

fn real_main() -> i32 {
    let raw: Vec<String> = std::env::args().collect();
    if raw.get(1).map(String::as_str) == Some("--version") {
        println!("0.1.0");
        return 0;
    }

    let cli = Cli::parse();
    let root = std::env::current_dir().unwrap_or_default();

    let result = match cli.command {
        Command::Init => cmd_init::run(&root),
        Command::Build => cmd_build::run(),
        Command::Gen {
            target,
            force,
            full,
            module,
        } => cmd_gen::run(
            &root,
            &cmd_gen::GenArgs {
                target: &target,
                force,
                full,
                module: module.as_deref(),
            },
        ),
        Command::Blame {
            file,
            line,
            gen,
            verbose,
        } => cmd_blame::run(
            &root,
            &cmd_blame::BlameArgs {
                file: file.as_deref(),
                line,
                gen,
                verbose,
            },
        ),
        Command::Reconcile { target, module } => cmd_reconcile::run(
            &root,
            &cmd_reconcile::ReconcileArgs {
                target: &target,
                module: module.as_deref(),
            },
        ),
        Command::Watch {
            target,
            module,
            debounce,
            once,
        } => cmd_watch::run(
            &root,
            &cmd_watch::WatchArgs {
                target: &target,
                module: module.as_deref(),
                debounce,
                once,
            },
        ),
        Command::Status => cmd_status::run(&root),
        Command::Test { target } => cmd_test::run(&root, &target),
        Command::Lsp => napl_lsp::run()
            .map(|()| 0)
            .map_err(|error| error::CliError::new(error.to_string())),
    };

    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("napl: {error}");
            1
        }
    }
}
