use anyhow::Result;
use colored::Colorize;
use fs_err as fs;
use std::path::{Path, PathBuf};

use crate::merge::{additive_merge, preserve_use_client, is_additive_task};
use crate::wire::{Plan, Step};

#[derive(Debug, Clone)]
pub enum ChangeKind { Create, Update, Delete, Command, Test }

#[derive(Debug, Clone)]
pub struct Preview {
    pub kind: ChangeKind,
    pub path: Option<PathBuf>,
    pub bytes_before: Option<u64>,
    pub bytes_after: Option<u64>,
    pub diff_snippet: Option<String>,
    pub command: Option<String>,
}

fn read_to_string_if_exists(path: &Path) -> Result<Option<String>> {
    if path.exists() {
        Ok(Some(fs::read_to_string(path)?))
    } else {
        Ok(None)
    }
}

fn short_diff(old: &str, new: &str, max_lines: usize) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;

    while (i < old_lines.len() || j < new_lines.len()) && out.len() < max_lines {
        if i < old_lines.len() && j < new_lines.len() && old_lines[i] == new_lines[j] {
            i += 1;
            j += 1;
            continue;
        }
        if i < old_lines.len() {
            out.push(format!("{}", format!("- {}", old_lines[i]).red()));
            i += 1;
        }
        if j < new_lines.len() {
            out.push(format!("{}", format!("+ {}", new_lines[j]).green()));
            j += 1;
        }
    }

    if out.len() >= max_lines {
        out.push("... (diff truncated)".dimmed().to_string());
    }
    out.join("\n")
}

pub fn preview(root: &Path, plan: &Plan, user_task: &str) -> Result<Vec<Preview>> {
    let mut previews = Vec::new();
    let additive = is_additive_task(user_task);

    for s in &plan.steps {
        match s {
            Step::Create { path, content, .. } => {
                let abs = root.join(path);
                let before = if abs.exists() { Some(abs.metadata()?.len()) } else { None };
                let after = content.as_ref().map(|c| c.as_bytes().len() as u64);
                let diff = match (read_to_string_if_exists(&abs)?, content) {
                    (Some(old), Some(new_model)) => {
                        let merged = preserve_use_client(Some(&old), new_model, user_task);
                        Some(short_diff(&old, &merged, 80))
                    }
                    _ => None,
                };
                previews.push(Preview {
                    kind: ChangeKind::Create,
                    path: Some(abs),
                    bytes_before: before,
                    bytes_after: after,
                    diff_snippet: diff,
                    command: None,
                });
            }
            Step::Update { path, content, .. } => {
                let abs = root.join(path);
                let before = if abs.exists() { Some(abs.metadata()?.len()) } else { None };
                let (after, diff) = match (read_to_string_if_exists(&abs)?, content) {
                    (Some(old), Some(new_model)) => {
                        let merged_base = if additive { additive_merge(&old, new_model) } else { new_model.clone() };
                        let merged = preserve_use_client(Some(&old), &merged_base, user_task);
                        let after = merged.as_bytes().len() as u64;
                        let diff = Some(short_diff(&old, &merged, 120));
                        (Some(after), diff)
                    }
                    _ => (None, None),
                };
                previews.push(Preview {
                    kind: ChangeKind::Update,
                    path: Some(abs),
                    bytes_before: before,
                    bytes_after: after,
                    diff_snippet: diff,
                    command: None,
                });
            }
            Step::Delete { path, .. } => {
                let abs = root.join(path);
                let before = if abs.exists() { Some(abs.metadata()?.len()) } else { Some(0) };
                previews.push(Preview {
                    kind: ChangeKind::Delete,
                    path: Some(abs),
                    bytes_before: before,
                    bytes_after: Some(0),
                    diff_snippet: None,
                    command: None,
                });
            }
            Step::Command { command, .. } => {
                previews.push(Preview {
                    kind: ChangeKind::Command,
                    path: None,
                    bytes_before: None,
                    bytes_after: None,
                    diff_snippet: None,
                    command: Some(command.clone()),
                });
            }
            Step::Test { command, .. } => {
                previews.push(Preview {
                    kind: ChangeKind::Test,
                    path: None,
                    bytes_before: None,
                    bytes_after: None,
                    diff_snippet: None,
                    command: Some(command.clone()),
                });
            }
        }
    }
    Ok(previews)
}

pub fn colorize_preview(p: &Preview) -> String {
    match p.kind {
        ChangeKind::Create => {
            format!(
                "{} {}  ({} -> {})\n{}",
                "[CREATE]".green().bold(),
                p.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                p.bytes_before.map(|b| format!("{b}B")).unwrap_or_else(|| "-".into()),
                p.bytes_after.map(|b| format!("{b}B")).unwrap_or_else(|| "-".into()),
                p.diff_snippet.clone().unwrap_or_default()
            )
        }
        ChangeKind::Update => {
            format!(
                "{} {}  ({} -> {})\n{}",
                "[UPDATE]".yellow().bold(),
                p.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                p.bytes_before.map(|b| format!("{b}B")).unwrap_or_else(|| "-".into()),
                p.bytes_after.map(|b| format!("{b}B")).unwrap_or_else(|| "-".into()),
                p.diff_snippet.clone().unwrap_or_default()
            )
        }
        ChangeKind::Delete => {
            format!(
                "{} {}  ({} -> {})",
                "[DELETE]".red().bold(),
                p.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                p.bytes_before.map(|b| format!("{b}B")).unwrap_or_else(|| "-".into()),
                p.bytes_after.map(|b| format!("{b}B")).unwrap_or_else(|| "-".into())
            )
        }
        ChangeKind::Command => {
            format!("{} {}", "[COMMAND]".cyan().bold(), p.command.clone().unwrap_or_default())
        }
        ChangeKind::Test => {
            format!("{} {}", "[TEST]".magenta().bold(), p.command.clone().unwrap_or_default())
        }
    }
}
