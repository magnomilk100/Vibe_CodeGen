use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CmdResult {
    pub command: String,
    pub status_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

pub fn run_command_allowlisted(
    cmd: &str,
    cfg: &Config,
    cwd: Option<&str>,
    timeout_secs: u64,
) -> Result<CmdResult> {
    if !cfg.command_allowlist.iter().any(|c| c == cmd) {
        return Err(anyhow!("command not in allowlist: {}", cmd));
    }

    let mut parts = cmd.split_whitespace();
    let program = parts
        .next()
        .ok_or_else(|| anyhow!("empty command"))?;
    let args: Vec<String> = parts.map(|s| s.to_string()).collect();

    let mut c = Command::new(program);
    c.args(&args);
    if let Some(cwd) = cwd {
        c.current_dir(cwd);
    }
    c.stdout(Stdio::piped());
    c.stderr(Stdio::piped());

    let start = Instant::now();
    let child = c
        .spawn()
        .with_context(|| format!("failed to spawn command {}", cmd))?;

    let timeout = Duration::from_secs(timeout_secs);
    let output = match wait_with_timeout::wait_with_timeout(child, timeout) {
        Ok(wait_with_timeout::WaitResult::Completed(out)) => out,
        Ok(wait_with_timeout::WaitResult::TimedOut) => {
            return Err(anyhow!(
                "command timed out after {}s: {}",
                timeout_secs,
                cmd
            ));
        }
        Err(e) => return Err(anyhow!("failed waiting for command: {}", e)),
    };

    let dur = start.elapsed();
    Ok(CmdResult {
        command: cmd.to_string(),
        status_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        duration_ms: dur.as_millis(),
    })
}

// Minimal blocking wait with timeout using a helper thread.
mod wait_with_timeout {
    use std::io;
    use std::process::{Child, Output};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    pub enum WaitResult {
        Completed(Output),
        TimedOut,
    }

    pub fn wait_with_timeout(child: Child, timeout: Duration) -> io::Result<WaitResult> {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let out = child.wait_with_output();
            let _ = tx.send(out);
        });

        match rx.recv_timeout(timeout) {
            Ok(Ok(output)) => Ok(WaitResult::Completed(output)),
            Ok(Err(e)) => Err(e),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(WaitResult::TimedOut),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
        }
    }
}
