//! Subprocess orchestration: the coding-agent runner, the LLM client, the test
//! command runner, and the gen lock. All shell out to real binaries; the
//! conformance corpus supplies deterministic stubs on `PATH`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::{CliError, CliResult};
use crate::fsutil;

/// The result of running the coding agent or the test command.
pub struct RunOutput {
    /// Combined stdout (+ stderr) text.
    pub output: String,
    /// The process exit code.
    pub code: i32,
}

fn build_agent_args(model: &str, allowed_tools: &[String]) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        "--output-format".to_string(),
        "text".to_string(),
        "--model".to_string(),
        model.to_string(),
        "--no-session-persistence".to_string(),
        "--permission-mode".to_string(),
        "acceptEdits".to_string(),
        "--allowedTools".to_string(),
    ];
    args.extend(allowed_tools.iter().cloned());
    args
}

fn run_with_stdin(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    stdin_text: &str,
) -> std::io::Result<(String, String, i32)> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_text.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    Ok((
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap_or(1),
    ))
}

/// Probe that the `claude` CLI exists, mirroring `requireClaudeAgent`/`requireClaudeCli`.
pub fn require_claude() -> CliResult<()> {
    let status = Command::new("claude")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(status) if status.success() => Ok(()),
        _ => Err(CliError::new(
            "the \"claude\" CLI was not found on PATH. Install Claude Code (claude.ai/code) — napl gen runs it as a coding agent.",
        )),
    }
}

/// Run the coding agent, mirroring `createClaudeAgentRunner().run`.
pub fn run_agent(
    task: &str,
    cwd: &Path,
    model: &str,
    allowed_tools: &[String],
) -> CliResult<RunOutput> {
    let args = build_agent_args(model, allowed_tools);
    let (stdout, stderr, code) = run_with_stdin("claude", &args, Some(cwd), task)
        .map_err(|e| CliError::new(format!("failed to spawn the \"claude\" agent: {e}")))?;
    let output = if stderr.trim().is_empty() {
        stdout
    } else {
        format!("{stdout}\n{stderr}")
    };
    Ok(RunOutput { output, code })
}

/// Complete an LLM request via the `claude` CLI, mirroring `createClaudeCliClient`.
pub fn llm_complete(model: &str, system: &str, user: &str) -> CliResult<String> {
    let mut args = vec![
        "-p".to_string(),
        "--output-format".to_string(),
        "text".to_string(),
        "--model".to_string(),
        model.to_string(),
        "--no-session-persistence".to_string(),
    ];
    if !system.trim().is_empty() {
        args.push("--system-prompt".to_string());
        args.push(system.to_string());
    }
    let (stdout, stderr, code) = run_with_stdin("claude", &args, None, user)
        .map_err(|e| CliError::new(format!("failed to spawn the \"claude\" CLI: {e}")))?;
    if code != 0 {
        let detail = if stderr.trim().is_empty() {
            "the claude CLI produced no stderr output".to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(CliError::new(format!(
            "the \"claude\" CLI exited with code {code}: {detail}"
        )));
    }
    if stdout.trim().is_empty() {
        return Err(CliError::new(
            "the \"claude\" CLI returned an empty response",
        ));
    }
    Ok(stdout)
}

/// Run the target's test command, mirroring `runCommand`.
pub fn run_command(command: &str, args: &[String], cwd: &Path) -> RunOutput {
    let result = Command::new(command)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match result {
        Ok(output) => {
            let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
            RunOutput {
                output: combined,
                code: output.status.code().unwrap_or(1),
            }
        }
        Err(error) => RunOutput {
            output: format!("\n{error}"),
            code: 1,
        },
    }
}

/// A held gen lock; [`GenLock::release`] removes it.
#[derive(Debug)]
pub struct GenLock {
    path: PathBuf,
}

impl GenLock {
    /// Remove the lock file if it still exists.
    pub fn release(&self) {
        if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn default_is_alive(pid: i32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Acquire the gen lock, mirroring `acquireGenLock` (pid + liveness injectable
/// for tests).
pub fn acquire_gen_lock_with(
    lock_path: &Path,
    pid: i32,
    is_alive: &dyn Fn(i32) -> bool,
) -> CliResult<GenLock> {
    fsutil::mkdir_parent(lock_path)?;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)
    {
        Ok(mut file) => {
            file.write_all(format!("{pid}\n").as_bytes())?;
            Ok(GenLock {
                path: lock_path.to_path_buf(),
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let held = held_pid(lock_path);
            if let Some(held) = held {
                if held != pid && is_alive(held) {
                    return Err(CliError::new(format!(
                        "another napl gen is already running (pid {held}); the lock {} is held. Wait for it to finish or remove the lock if the process is gone.",
                        lock_path.to_string_lossy()
                    )));
                }
            }
            std::fs::write(lock_path, format!("{pid}\n"))?;
            Ok(GenLock {
                path: lock_path.to_path_buf(),
            })
        }
        Err(error) => Err(CliError::new(format!(
            "could not acquire gen lock at {}: {error}",
            lock_path.to_string_lossy()
        ))),
    }
}

/// Acquire the gen lock with the real process id and liveness check.
pub fn acquire_gen_lock(lock_path: &Path) -> CliResult<GenLock> {
    let pid = i32::try_from(std::process::id()).unwrap_or(0);
    acquire_gen_lock_with(lock_path, pid, &default_is_alive)
}

fn held_pid(lock_path: &Path) -> Option<i32> {
    let raw = std::fs::read_to_string(lock_path).ok()?;
    raw.trim().parse::<i32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_creates_lock_and_release_removes_it() {
        let dir = std::env::temp_dir().join(format!("napl-lock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gen.lock");
        let _ = std::fs::remove_file(&path);
        let lock = acquire_gen_lock_with(&path, 4242, &|_| false).unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "4242\n");
        lock.release();
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn contention_when_held_by_live_other_pid() {
        let dir = std::env::temp_dir().join(format!("napl-lock-c-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gen.lock");
        std::fs::write(&path, "9999\n").unwrap();
        let result = acquire_gen_lock_with(&path, 4242, &|_| true);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .0
            .contains("another napl gen is already running (pid 9999)"));
        // lock must be untouched
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "9999\n");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stale_lock_is_stolen_when_holder_dead() {
        let dir = std::env::temp_dir().join(format!("napl-lock-s-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gen.lock");
        std::fs::write(&path, "9999\n").unwrap();
        let lock = acquire_gen_lock_with(&path, 4242, &|_| false).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "4242\n");
        lock.release();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn same_pid_reacquires_without_contention() {
        let dir = std::env::temp_dir().join(format!("napl-lock-r-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gen.lock");
        std::fs::write(&path, "4242\n").unwrap();
        let lock = acquire_gen_lock_with(&path, 4242, &|_| true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "4242\n");
        lock.release();
        std::fs::remove_dir_all(&dir).ok();
    }
}
