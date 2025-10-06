use anyhow::{anyhow, Context, Result};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::config::Config;

/// Result of a single command execution.
#[derive(Debug, Clone)]
pub struct CmdResult {
    pub command: String,
    pub cwd: Option<String>,
    /// Legacy field kept for backward-compat with existing UX code.
    pub status: i32,
    /// Canonical status code (same as `status`).
    pub status_code: i32,
    pub stdout: String,
    pub stderr: String,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u128,
    /// True if we had to fall back to a platform shell to execute.
    pub via_shell_fallback: bool,
}

impl Default for CmdResult {
    fn default() -> Self {
        Self {
            command: String::new(),
            cwd: None,
            status: 0,
            status_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 0,
            via_shell_fallback: false,
        }
    }
}

/// Naive, shell-like splitter that respects single and double quotes.
/// "npm install" => ["npm","install"]
/// `pnpm add "react-dom@^18"` => ["pnpm","add","react-dom@^18"]
fn split_cmdline(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut in_quote: Option<char> = None;

    for c in s.chars() {
        match (in_quote, c) {
            (Some(q), ch) if ch == q => in_quote = None,
            (Some(_), ch) => buf.push(ch),
            (None, '"') => in_quote = Some('"'),
            (None, '\'') => in_quote = Some('\''),
            (None, ch) if ch.is_whitespace() => {
                if !buf.is_empty() {
                    out.push(buf.clone());
                    buf.clear();
                }
            }
            (None, ch) => buf.push(ch),
        }
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

/// Build a Command configured with cwd and IO capture.
fn build_command(program: &str, args: &[String], cwd: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

/// Run the given command string if and only if it's exactly in the allowlist.
/// The `cwd_opt` overrides working directory relative to project root.
/// `_timeout_secs` accepted for API compatibility (no enforced timeout here).
pub fn run_command_allowlisted(
    cmd_str: &str,
    cfg: &Config,
    cwd_opt: Option<&str>,
    _timeout_secs: u64,
) -> Result<CmdResult> {
    if !cfg.command_allowlist.iter().any(|c| c == cmd_str) {
        return Err(anyhow!(
            "command '{}' is not in allowlist: {:?}",
            cmd_str,
            cfg.command_allowlist
        ));
    }

    let root = Path::new(&cfg.root);
    let cwd_path: PathBuf = match cwd_opt {
        Some(rel) if !rel.trim().is_empty() => root.join(rel),
        _ => root.to_path_buf(),
    };
    let cwd_disp = cwd_path.display().to_string();

    let parts = split_cmdline(cmd_str);
    if parts.is_empty() {
        return Err(anyhow!("empty command"));
    }
    let program = &parts[0];
    let args: Vec<String> = parts[1..].to_vec();

    let started = Instant::now();

    // Attempt direct spawn first
    let output_direct = build_command(program, &args, &cwd_path).output();

    let (output, via_shell_fallback) = match output_direct {
        Ok(out) => (out, false),
        Err(e) => {
            // If program not found, try shell fallback (Windows or Unix)
            if e.kind() == io::ErrorKind::NotFound {
                #[cfg(windows)]
                {
                    let out = Command::new("cmd")
                        .arg("/C")
                        .arg(cmd_str)
                        .current_dir(&cwd_path)
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output()
                        .with_context(|| format!("shell fallback failed for '{}'", cmd_str))?;
                    (out, true)
                }
                #[cfg(not(windows))]
                {
                    let out = Command::new("sh")
                        .arg("-lc")
                        .arg(cmd_str)
                        .current_dir(&cwd_path)
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output()
                        .with_context(|| format!("shell fallback failed for '{}'", cmd_str))?;
                    (out, true)
                }
            } else {
                return Err(anyhow!("failed to spawn command '{}': {}", cmd_str, e));
            }
        }
    };

    let elapsed = started.elapsed().as_millis();

    let code = output
        .status
        .code()
        .unwrap_or_else(|| if output.status.success() { 0 } else { -1 });

    let res = CmdResult {
        command: cmd_str.to_string(),
        cwd: Some(cwd_disp),
        status: code,
        status_code: code,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration_ms: elapsed,
        via_shell_fallback,
    };

    if code != 0 {
        return Err(anyhow!(
            "command exited with non-zero status {}\nstdout:\n{}\nstderr:\n{}",
            code,
            res.stdout,
            res.stderr
        ));
    }

    Ok(res)
}
