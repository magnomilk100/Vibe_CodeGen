use anyhow::{anyhow, Context, Result};
use fs_err as fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::exec::{run_command_allowlisted, CmdResult};
use crate::merge;
use crate::wire::Step;

#[derive(Debug, Clone)]
pub struct ApplySummary {
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
    pub commands: usize,
    pub tests: usize,
    pub skipped: usize,
    pub bytes: usize,
    pub command_outputs: Vec<CmdResult>,
}

impl Default for ApplySummary {
    fn default() -> Self {
        Self {
            created: 0,
            updated: 0,
            deleted: 0,
            commands: 0,
            tests: 0,
            skipped: 0,
            bytes: 0,
            command_outputs: vec![],
        }
    }
}

pub fn apply_steps(
    root: &Path,
    steps: &[Step],
    dry_run: bool,
    cfg: &Config,
    task: &str,
) -> Result<ApplySummary> {
    let mut summary = ApplySummary::default();

    for step in steps {
        match step {
            Step::Create {
                path,
                content,
                ..
            } => {
                let abs = safe_join(root, path, &cfg.path_allowlist)
                    .with_context(|| format!("create path rejected: {}", path))?;
                let data = content
                    .as_ref()
                    .ok_or_else(|| anyhow!("create step missing content for {}", path))?;
                if dry_run {
                    summary.created += 1;
                    summary.bytes += data.as_bytes().len();
                    continue;
                }
                write_atomic(&abs, data)?;
                summary.created += 1;
                summary.bytes += data.as_bytes().len();
            }

            Step::Update {
                path,
                content,
                patch,
                ..
            } => {
                let abs = safe_join(root, path, &cfg.path_allowlist)
                    .with_context(|| format!("update path rejected: {}", path))?;
                if content.is_none() && patch.is_none() {
                    // Nothing to do
                    summary.skipped += 1;
                    continue;
                }

                // Prefer full content updates when provided
                if let Some(new_content) = content {
                    // If additive task, merge on top of the old base
                    if abs.exists() && abs.is_file() {
                        let old = fs::read_to_string(&abs).unwrap_or_default();
                        let mut final_content = new_content.clone();

                        // preserve 'use client' if the old file had it
                        final_content = merge::preserve_use_client(Some(&old), &final_content, task);

                        // perform additive merge if task looks additive and file is ts/tsx
                        let looks_additive = merge::is_additive_task(task)
                            && (path.ends_with(".tsx") || path.ends_with(".ts") || path.ends_with(".js"));
                        if looks_additive {
                            let merged = merge::additive_merge(&old, &final_content);
                            final_content = merged;
                        }

                        if dry_run {
                            summary.updated += 1;
                            summary.bytes += final_content.as_bytes().len();
                        } else {
                            write_atomic(&abs, &final_content)?;
                            summary.updated += 1;
                            summary.bytes += final_content.as_bytes().len();
                        }
                    } else {
                        // No old file; this is effectively a create
                        if dry_run {
                            summary.created += 1;
                            summary.bytes += new_content.as_bytes().len();
                        } else {
                            write_atomic(&abs, new_content)?;
                            summary.created += 1;
                            summary.bytes += new_content.as_bytes().len();
                        }
                    }
                } else if let Some(_patch) = patch {
                    // Patch-only path — for now, we do a conservative skip (preview handled elsewhere)
                    // You may integrate a real unified-diff applier here in the future.
                    summary.skipped += 1;
                }
            }

            Step::Delete { path, .. } => {
                let abs = safe_join(root, path, &cfg.path_allowlist)
                    .with_context(|| format!("delete path rejected: {}", path))?;
                if dry_run {
                    if abs.exists() {
                        summary.deleted += 1;
                    } else {
                        summary.skipped += 1;
                    }
                    continue;
                }
                if abs.exists() {
                    fs::remove_file(&abs).with_context(|| format!("failed to delete {}", path))?;
                    summary.deleted += 1;
                } else {
                    summary.skipped += 1;
                }
            }

            Step::Command { command, cwd, .. } => {
                summary.commands += 1;
                if dry_run {
                    // synthesize a placeholder result compatible with dashboards
                    let mut placeholder = CmdResult::default();
                    placeholder.command = command.clone();
                    placeholder.cwd = Some(cwd.clone().unwrap_or_else(|| ".".into()));
                    placeholder.status = 0;
                    placeholder.status_code = 0;
                    placeholder.duration_ms = 0;
                    placeholder.via_shell_fallback = false;
                    summary.command_outputs.push(placeholder);
                } else {
                    let res = run_command_allowlisted(command, cfg, cwd.as_deref(), cfg.timeout_secs)
                        .with_context(|| format!("command failed: {}", command))?;
                    summary.command_outputs.push(res);
                }
            }

            Step::Test { command, .. } => {
                summary.tests += 1;
                if dry_run {
                    let mut placeholder = CmdResult::default();
                    placeholder.command = command.clone();
                    placeholder.cwd = Some(".".into());
                    placeholder.status = 0;
                    placeholder.status_code = 0;
                    placeholder.duration_ms = 0;
                    placeholder.via_shell_fallback = false;
                    summary.command_outputs.push(placeholder);
                } else {
                    // Re-use allowlisted runner; if tests aren't allowlisted, skip with warning semantics
                    if cfg.command_allowlist.iter().any(|c| c == command) {
                        let res = run_command_allowlisted(command, cfg, None, cfg.timeout_secs)
                            .with_context(|| format!("test command failed: {}", command))?;
                        summary.command_outputs.push(res);
                    } else {
                        // not in allowlist -> skip, but log a placeholder
                        let mut placeholder = CmdResult::default();
                        placeholder.command = format!("(skipped-not-allowlisted) {}", command);
                        placeholder.cwd = Some(".".into());
                        placeholder.status = 0;
                        placeholder.status_code = 0;
                        placeholder.duration_ms = 0;
                        placeholder.via_shell_fallback = false;
                        summary.command_outputs.push(placeholder);
                        summary.skipped += 1;
                    }
                }
            }
        }
    }

    Ok(summary)
}

/// Join `root` with a relative path, enforcing a simple allowlist prefix guard.
fn safe_join(root: &Path, rel: &str, allowlist: &[String]) -> Result<PathBuf> {
    // quick allowlist prefix check (top-level segments)
    let allowed = allowlist.iter().any(|p| {
        if p == rel { return true; }
        rel.starts_with(p.trim_end_matches('/').trim_end_matches('\\'))
    });
    if !allowed {
        return Err(anyhow!("path '{}' not allowed by allowlist", rel));
    }

    let candidate = root.join(rel);
    let can = candidate
        .canonicalize()
        .unwrap_or_else(|_| candidate.clone());
    let root_can = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf());

    if !can.starts_with(&root_can) {
        return Err(anyhow!("path escapes project root: {}", rel));
    }
    Ok(candidate)
}

/// Atomic write with directory creation.
fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .with_context(|| format!("failed to create dir {}", dir.display()))?;
    }

    // Ensure trailing newline per hygiene rule when writing text files
    let final_contents = if contents.ends_with('\n') {
        contents.to_string()
    } else {
        let mut s = contents.to_string();
        s.push('\n');
        s
    };

    // Write to a temp file then rename
    let tmp = path.with_extension(".__tmp__");
    {
        let mut f = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)
            .with_context(|| format!("open temp for write: {}", tmp.display()))?;
        f.write_all(final_contents.as_bytes())
            .with_context(|| format!("write temp: {}", tmp.display()))?;
        f.flush()?;
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}
