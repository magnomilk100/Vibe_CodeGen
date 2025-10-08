use anyhow::{bail, Context, Result};
use std::io;
use std::process::{Command, Stdio};

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct CmdResult {
    pub command: String,
    pub cwd: Option<String>,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
    pub via_shell_fallback: bool,
}

pub fn run_command_allowlisted(
    cmd: &str,
    cfg: &Config,
    cwd: Option<&str>,
    timeout_secs: u64,
) -> Result<CmdResult> {
    if !crate::safety::command_is_allowed(cmd, &cfg.command_allowlist) {
        bail!(
            "command not allowed: {} (allowlist: {:?})",
            cmd,
            cfg.command_allowlist
        );
    }

    // Try direct spawn first
    match run_direct(cmd, cwd, timeout_secs) {
        Ok(r) => return Ok(r),
        Err(e) => {
            // On Windows (and sometimes on *nix) complex commands with args
            // may require shell. Fallback to shell execution.
            let shell_cmd = shell_fallback(cmd, cwd, timeout_secs)
                .with_context(|| format!("failed to spawn command via shell: {}", cmd))?;
            if shell_cmd.status != 0 {
                bail!("command failed ({}):\nSTDOUT:\n{}\nSTDERR:\n{}", cmd, shell_cmd.stdout, shell_cmd.stderr);
            }
            return Ok(shell_cmd);
        }
    }
}

fn run_direct(cmd: &str, cwd: Option<&str>, _timeout_secs: u64) -> Result<CmdResult> {
    // Split command into program + args (simple split by whitespace)
    let mut parts = shlex::Shlex::new(cmd);
    let mut tokens: Vec<String> = parts.by_ref().collect();
    if tokens.is_empty() {
        bail!("empty command");
    }
    let program = tokens.remove(0);

    let mut c = Command::new(program);
    if let Some(dir) = cwd {
        c.current_dir(dir);
    }
    c.args(tokens);
    c.stdout(Stdio::piped()).stderr(Stdio::piped());

    let out = c.output().with_context(|| format!("failed to spawn command {}", cmd))?;

    Ok(CmdResult {
        command: cmd.to_string(),
        cwd: cwd.map(|s| s.to_string()),
        status: out.status.code().unwrap_or_default(),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        via_shell_fallback: false,
    })
}

#[cfg(target_os = "windows")]
fn shell_fallback(cmd: &str, cwd: Option<&str>, _timeout_secs: u64) -> Result<CmdResult> {
    let mut c = Command::new("cmd");
    c.arg("/C").arg(cmd);
    if let Some(dir) = cwd {
        c.current_dir(dir);
    }
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = c.output()?;

    Ok(CmdResult {
        command: cmd.to_string(),
        cwd: cwd.map(|s| s.to_string()),
        status: out.status.code().unwrap_or_default(),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        via_shell_fallback: true,
    })
}

#[cfg(not(target_os = "windows"))]
fn shell_fallback(cmd: &str, cwd: Option<&str>, _timeout_secs: u64) -> Result<CmdResult> {
    let mut c = Command::new("sh");
    c.arg("-lc").arg(cmd);
    if let Some(dir) = cwd {
        c.current_dir(dir);
    }
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = c.output()?;

    Ok(CmdResult {
        command: cmd.to_string(),
        cwd: cwd.map(|s| s.to_string()),
        status: out.status.code().unwrap_or_default(),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        via_shell_fallback: true,
    })
}
