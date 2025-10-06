use crate::config::Config;
use crate::wire::{Plan, Step};
use anyhow::{anyhow, Result};
use std::path::{Component, Path, PathBuf};

fn normalize_relative(p: &str) -> Result<PathBuf> {
    let path = Path::new(p);
    if path.is_absolute() {
        return Err(anyhow!("absolute paths are not allowed: {}", p));
    }
    let mut buf = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => {
                return Err(anyhow!("absolute path components not allowed: {}", p))
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(anyhow!("parent traversal not allowed: {}", p));
            }
            Component::Normal(s) => buf.push(s),
        }
    }
    Ok(buf)
}

fn is_path_allowed(rel: &Path, allowlist: &[String]) -> bool {
    // A path is allowed if it starts with one of the allowlist prefixes
    // (component-aware).
    for allow in allowlist {
        let allow_path = Path::new(allow);
        if rel.starts_with(allow_path) || rel == allow_path {
            return true;
        }
    }
    false
}

fn is_command_allowed(cmd: &str, allowlist: &[String]) -> bool {
    allowlist.iter().any(|c| c == cmd)
}

pub fn validate(plan: &Plan, cfg: &Config) -> Result<()> {
    // Count guard
    if plan.steps.len() > cfg.max_actions {
        return Err(anyhow!(
            "too many steps: {} > max_actions {}",
            plan.steps.len(),
            cfg.max_actions
        ));
    }

    let mut total_bytes: usize = 0;

    for s in &plan.steps {
        match s {
            Step::Create { path, content, .. } => {
                let rel = normalize_relative(path)?;
                if !is_path_allowed(&rel, &cfg.path_allowlist) {
                    return Err(anyhow!(
                        "path not allowed for CREATE: {} (allowlist: {:?})",
                        path,
                        cfg.path_allowlist
                    ));
                }
                if let Some(c) = content {
                    total_bytes += c.as_bytes().len();
                }
            }
            Step::Update { path, patch, content, .. } => {
                let rel = normalize_relative(path)?;
                if !is_path_allowed(&rel, &cfg.path_allowlist) {
                    return Err(anyhow!(
                        "path not allowed for UPDATE: {} (allowlist: {:?})",
                        path,
                        cfg.path_allowlist
                    ));
                }
                if let Some(c) = content {
                    total_bytes += c.as_bytes().len();
                }
                if let Some(p) = patch {
                    total_bytes += p.as_bytes().len();
                }
            }
            Step::Delete { path, .. } => {
                let rel = normalize_relative(path)?;
                if !is_path_allowed(&rel, &cfg.path_allowlist) {
                    return Err(anyhow!(
                        "path not allowed for DELETE: {} (allowlist: {:?})",
                        path,
                        cfg.path_allowlist
                    ));
                }
            }
            Step::Command { command, .. } | Step::Test { command, .. } => {
                if !is_command_allowed(command, &cfg.command_allowlist) {
                    return Err(anyhow!(
                        "command not allowed: {} (allowlist: {:?})",
                        command,
                        cfg.command_allowlist
                    ));
                }
            }
        }
    }

    if total_bytes > cfg.max_patch_bytes {
        return Err(anyhow!(
            "total planned payload {} bytes exceeds limit {} bytes",
            total_bytes,
            cfg.max_patch_bytes
        ));
    }

    Ok(())
}
