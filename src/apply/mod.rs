use crate::exec::{run_command_allowlisted, CmdResult};
use crate::merge::{additive_merge, preserve_use_client, is_additive_task};
use crate::wire::Step;
use anyhow::{anyhow, Result};
use fs_err as fs;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

#[derive(Debug, Clone)]
pub enum ApplyKind { Created, Updated, Deleted, Command, Test, Skipped }

#[derive(Debug, Clone)]
pub struct FileResult {
    pub kind: ApplyKind,
    pub path: Option<PathBuf>,
    pub bytes_before: Option<u64>,
    pub bytes_after: Option<u64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ApplySummary {
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
    pub commands_run: usize,
    pub tests_run: usize,
    pub skipped: usize,
    pub bytes_written: u64,
    pub details: Vec<FileResult>,
    pub command_outputs: Vec<CmdResult>,
}

impl ApplySummary {
    pub fn new() -> Self {
        Self {
            created: 0, updated: 0, deleted: 0, commands_run: 0, tests_run: 0,
            skipped: 0, bytes_written: 0, details: Vec::new(), command_outputs: Vec::new()
        }
    }
}

pub fn apply_steps(root: &Path, steps: &[Step], dry: bool, cfg: &crate::config::Config, user_task: &str) -> Result<ApplySummary> {
    let mut sum = ApplySummary::new();
    let additive = is_additive_task(user_task);

    for s in steps {
        match s {
            Step::Create { path, content, .. } => {
                let abs = root.join(path);
                let before = if abs.exists() { Some(abs.metadata()?.len()) } else { None };
                let data = content.as_ref().ok_or_else(|| anyhow!("CREATE requires 'content' for {}", path))?;
                let after = data.as_bytes().len() as u64;

                if !dry {
                    if let Some(parent) = abs.parent() { fs::create_dir_all(parent)?; }
                    let tmp = NamedTempFile::new_in(abs.parent().unwrap_or(root))?;
                    fs::write(tmp.path(), data)?;
                    tmp.persist(&abs)?;
                }

                sum.created += if before.is_none() { 1 } else { 0 };
                sum.updated += if before.is_some() { 1 } else { 0 };
                sum.bytes_written += after;
                sum.details.push(FileResult {
                    kind: if before.is_none() { ApplyKind::Created } else { ApplyKind::Updated },
                    path: Some(abs),
                    bytes_before: before,
                    bytes_after: Some(after),
                    note: None,
                });
            }
            Step::Update { path, content, patch, .. } => {
                let abs = root.join(path);
                let before = if abs.exists() { Some(abs.metadata()?.len()) } else { None };

                if let Some(proposed) = content {
                    let old_text = if abs.exists() { Some(fs::read_to_string(&abs)?) } else { None };
                    let merged = if additive && old_text.is_some() {
                        additive_merge(old_text.as_deref().unwrap(), proposed)
                    } else {
                        proposed.to_owned()
                    };
                    let final_content = preserve_use_client(old_text.as_deref(), &merged, user_task);
                    let after = final_content.as_bytes().len() as u64;

                    if !dry {
                        if let Some(parent) = abs.parent() { fs::create_dir_all(parent)?; }
                        let overwrite = NamedTempFile::new_in(abs.parent().unwrap_or(root))?;
                        fs::write(overwrite.path(), final_content)?;
                        overwrite.persist(&abs)?;
                    }
                    sum.updated += 1;
                    sum.bytes_written += after;
                    sum.details.push(FileResult {
                        kind: ApplyKind::Updated,
                        path: Some(abs),
                        bytes_before: before,
                        bytes_after: Some(after),
                        note: if additive { Some("applied additive merge".into()) } else { None },
                    });
                } else if let Some(_u_patch) = patch {
                    // For reliability we require 'content'. Skip patch-only updates.
                    sum.skipped += 1;
                    sum.details.push(FileResult {
                        kind: ApplyKind::Skipped,
                        path: Some(abs),
                        bytes_before: before,
                        bytes_after: before,
                        note: Some("update skipped: 'patch' only; please regenerate with full 'content' for this file".into()),
                    });
                } else {
                    return Err(anyhow!("UPDATE requires either 'content' or 'patch' for {}", path));
                }
            }
            Step::Delete { path, .. } => {
                let abs = root.join(path);
                let before = if abs.exists() { Some(abs.metadata()?.len()) } else { None };
                if !dry {
                    if abs.exists() {
                        fs::remove_file(&abs)?;
                    }
                }
                sum.deleted += 1;
                sum.details.push(FileResult {
                    kind: ApplyKind::Deleted,
                    path: Some(abs),
                    bytes_before: before,
                    bytes_after: Some(0),
                    note: None,
                });
            }
            Step::Command { command, cwd, .. } => {
                let res = if dry {
                    crate::exec::CmdResult {
                        command: command.clone(),
                        status_code: 0,
                        stdout: String::from("[dry-run] command not executed"),
                        stderr: String::new(),
                        duration_ms: 0,
                    }
                } else {
                    run_command_allowlisted(command, cfg, cwd.as_deref(), cfg.timeout_secs)?
                };
                sum.commands_run += 1;
                sum.command_outputs.push(res);
                sum.details.push(FileResult {
                    kind: ApplyKind::Command,
                    path: None,
                    bytes_before: None,
                    bytes_after: None,
                    note: None,
                });
            }
            Step::Test { command, .. } => {
                let res = if dry {
                    crate::exec::CmdResult {
                        command: command.clone(),
                        status_code: 0,
                        stdout: String::from("[dry-run] test not executed"),
                        stderr: String::new(),
                        duration_ms: 0,
                    }
                } else {
                    run_command_allowlisted(command, cfg, None, cfg.timeout_secs)?
                };
                sum.tests_run += 1;
                sum.command_outputs.push(res);
                sum.details.push(FileResult {
                    kind: ApplyKind::Test,
                    path: None,
                    bytes_before: None,
                    bytes_after: None,
                    note: None,
                });
            }
        }
    }

    Ok(sum)
}
